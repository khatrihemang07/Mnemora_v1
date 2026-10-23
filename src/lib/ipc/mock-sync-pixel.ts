/**
 * "Pixel 8 (simulated)" for the browser mock: a second device with its own copy of the data that
 * makes edits and syncs with the mock's data, running the TypeScript copy of the merge rules
 * (mock-sync-merge.ts, guarded by fixtures/sync-merge-rules.json). What the merge finds shows up on
 * the Sync pages as Rust would show it: Conflicts, Resolved automatically, Activity, toasts, and
 * the Todo and Journal views refreshing. Dev only: the mock is never part of a production build.
 *
 * Console (`window.__pixel`):
 *   __pixel.edit('task_edit', { task: 'Call mum', title: 'Call mum at 6' })   the phone makes an edit
 *   __pixel.sync()                       the phone and this device sync (the first call also
 *                                        starts them from the same data, as if paired earlier)
 *   __pixel.scenario('conflict')         stage a prototype scenario (see the list in mock.ts)
 *   __pixel.world()                      what the phone shows now
 * Ops (fixtures/sync-merge-rules.json): list_create(list,name) list_rename(list,name) list_delete(list)
 * task_create(task,title,list?,notes?) task_edit(task,title?,notes?) task_priority(task,priority)
 * task_when(task,date,time) task_done(task) task_move(task,list|null) task_delete(task)
 * task_place(task,before|null) comment_add(comment,task,body) comment_edit(comment,body)
 * entry_create(entry,body) entry_edit(entry,body) pref(key,value). A Task, List, Entry or Comment is
 * named by its id or by its title, name or the start of its text.
 */
import type {
  ConflictRow,
  ConflictSubject,
  ResolvedIcon,
  ResolvedRow,
  TodoListsMergedEvent,
  TodoTaskDeletedEvent,
} from './bindings';
import { subjectWords } from './mock-sync-activity';
import type { Sample } from './mock-sync-conflicts';
import { type Doc, fold, type Json, type Outcome, type TextClash } from './mock-sync-merge';
import { capture, createIds, createReplica, perform, syncReplicas } from './mock-sync-replica';

export const PIXEL = { id: 'mock-pixel', name: 'Pixel 8 (simulated)' };
const ME_ID = 'mock-me';

export interface PixelPorts {
  /** Todo and Journal: their data as docs, and taking merged docs back (true if it changed anything). */
  data: { todo: DataPort; journal: DataPort };
  /** Preferences (not look and feel): docs, and taking merged ones back (returns the keys changed). */
  settings: { exportDocs(): Doc[]; importDocs(docs: Doc[]): string[] };
  /** Runs a mock command, for staging scenarios. */
  command(cmd: string, args: Record<string, unknown>): unknown;
  conflicts: {
    raise(sample: Sample): boolean;
    addResolved(rows: ResolvedRow[]): void;
  };
  activity: { add(...events: ActivityEvent[]): void };
  /** This device's name in Sync. */
  myName(): string;
  announce: {
    todoChanged(): void;
    journalChanged(): void;
    listsMerged(e: TodoListsMergedEvent): void;
    taskDeleted(e: TodoTaskDeletedEvent): void;
    activityChanged(): void;
    settingsChanged(module: string, keys: string[]): void;
    newConflicts(count: number, device: string): void;
  };
  now?: () => number;
}

interface DataPort {
  exportDocs(): Doc[];
  importDocs(docs: Doc[]): boolean;
}

export interface ActivityEvent {
  id: string;
  kind: string;
  at: number;
  device: string;
  peer: string;
  n_in: number;
  n_out: number;
  breakdown: string | null;
  extra: Record<string, string>;
}

const hash = (text: string) => {
  let h = 0;
  for (const c of text) h = (Math.imul(h, 31) + c.charCodeAt(0)) | 0;
  return (h >>> 0).toString(36);
};

const NOUNS: Record<string, [string, string, string]> = {
  task: ['Todo', 'Task', 'Tasks'],
  list: ['Todo', 'List', 'Lists'],
  comment: ['Todo', 'Comment', 'Comments'],
  entry: ['Journal', 'Entry', 'Entries'],
  setting: ['Settings', '', ''],
};

/** "Todo: 9 Tasks, 1 List · Journal: 2 Entries · Settings: 1" for these records (Rust `breakdown_text`). */
export function breakdown(entities: string[]): string | null {
  const counts = new Map<string, number>();
  for (const e of entities) counts.set(e, (counts.get(e) ?? 0) + 1);
  const modules = new Map<string, string[]>();
  for (const [entity, n] of counts) {
    const [module, one, many] = NOUNS[entity] as [string, string, string];
    modules.set(module, [
      ...(modules.get(module) ?? []),
      one ? `${n} ${n === 1 ? one : many}` : `${n}`,
    ]);
  }
  if (modules.size === 0) return null;
  return [...modules].map(([m, words]) => `${m}: ${words.join(', ')}`).join(' · ');
}

const UNIT_WORDS: Record<string, [string, ResolvedIcon]> = {
  priority: ['Priority', 'flag'],
  when: ['Due date', 'calendar'],
  state: ['Status', 'check'],
  list_id: ['List', 'list'],
  name: ['Name', 'list'],
  emoji: ['Emoji', 'list'],
  page_date: ['Page', 'other'],
  value: ['Preference', 'order'],
};

export function createPixelMock(ports: PixelPorts) {
  const now = ports.now ?? Date.now;
  const me = createReplica(ME_ID);
  const phone = createReplica(PIXEL.id);
  const ids =
    createIds(
      '-pixel-',
    ) /* '-' sorts below a uuid: the phone's ids read as the older, like Rust's time-ordered ids */;
  let started = false;
  let counter = 0;
  /** What was already shown, so a sync only adds what is new. */
  const known = new Set<string>();

  const deviceName = (id: string) => (id === PIXEL.id ? PIXEL.name : ports.myName());
  const localDocs = (): Doc[] => [
    ...ports.data.todo.exportDocs(),
    ...ports.data.journal.exportDocs(),
    ...ports.settings.exportDocs(),
  ];

  /** Both devices start from the same data, as if they had been paired and synced before. */
  const rebase = () => {
    started = true;
    capture(me, localDocs(), now());
    syncReplicas(me, phone);
  };
  const start = () => {
    if (!started) rebase();
  };

  const docs = (o: Outcome, entity: string) => o.docs.filter((d) => d.entity === entity);
  const titleOf = (o: Outcome, entity: string, id: string): string => {
    const d = o.docs.find((x) => x.entity === entity && x.id === id);
    return d ? String(d.values.title ?? d.values.name ?? d.values.body ?? '') : '';
  };

  /** Finds what a person names: an id, or a Task's title, a List's name, or the start of a text. */
  const find = (entity: string, name: unknown): string | undefined => {
    if (typeof name !== 'string') return undefined;
    const o = fold(phone.log);
    const live = docs(o, entity).filter((d) => d.values.deleted_at == null);
    const text = name.toLowerCase();
    return (
      live.find((d) => d.id === name)?.id ??
      live.find((d) =>
        String(d.values.title ?? d.values.name ?? d.values.body ?? '')
          .toLowerCase()
          .startsWith(text),
      )?.id
    );
  };

  const KEYS: Record<string, string> = {
    task: 'task',
    list: 'list',
    before: 'task',
    comment: 'comment',
    entry: 'entry',
  };
  const CREATES: Record<string, string> = {
    list_create: 'list',
    task_create: 'task',
    comment_add: 'comment',
    entry_create: 'entry',
  };

  /** The phone makes an edit. Returns the id of what it made, if it made something. */
  function edit(op: string, args: Record<string, unknown> = {}) {
    start();
    const step: Record<string, unknown> = { op, ...args };
    for (const [key, entity] of Object.entries(KEYS)) {
      if (CREATES[op] === entity && key === entity) continue;
      const id = find(entity, step[key]);
      if (id) step[key] = id;
    }
    // Always after what this device has written so far: the phone is the later of the two.
    return perform(phone, step as never, ids, Math.max(now(), me.lastMs + 1));
  }

  const subjectOf = (c: TextClash, o: Outcome): ConflictSubject => {
    if (c.entity === 'entry') {
      const entry = o.docs.find((d) => d.entity === 'entry' && d.id === c.id);
      return {
        kind: 'entry',
        page_date: String(entry?.values.page_date ?? ''),
        written_at: Number(entry?.values.written_at ?? c.theirsAt),
        written_offset_min: Number(
          entry?.values.written_offset_min ??
            -new Date(Number(entry?.values.written_at ?? c.theirsAt)).getTimezoneOffset(),
        ),
      };
    }
    if (c.entity === 'comment') {
      const comment = o.docs.find((d) => d.entity === 'comment' && d.id === c.id);
      return { kind: 'comment', task_title: titleOf(o, 'task', String(comment?.values.task_id)) };
    }
    return {
      kind: 'task',
      title: titleOf(o, 'task', c.id),
      field: c.field === 'title' ? 'title' : 'notes',
    };
  };

  const conflictOf = (
    c: TextClash,
    o: Outcome,
    write: (text: string) => void,
    live: () => string | undefined,
  ): Sample => {
    const id = `pixel-conflict-${hash(`${c.entity}/${c.id}/${c.field}/${c.base}/${c.ours}/${c.theirs}`)}`;
    const row: ConflictRow = {
      id,
      subject: subjectOf(c, o),
      module: c.entity === 'entry' ? 'journal' : 'todo',
      raised_at: c.theirsAt,
      showing: { device: deviceName(c.theirsDevice), at: c.theirsAt },
      other: { device: deviceName(c.oursDevice), at: c.oursAt },
    };
    return {
      row,
      base: c.base,
      showing: c.theirs,
      other: c.ours,
      open: true,
      resolution: null,
      apply: write,
      live,
    };
  };

  const show = (unit: string, v: Record<string, Json>): string => {
    if (unit === 'priority') return `P${String(v.priority)}`;
    if (unit === 'when') return [v.due_date, v.due_time].filter(Boolean).join(' ') || 'no date';
    if (unit === 'state') return String(v.state);
    return String(Object.values(v)[0] ?? '');
  };

  /** Rows for "Resolved automatically" from what a sync found. */
  const resolvedRows = (
    o: Outcome,
    fresh: {
      values: Outcome['values'];
      deleteEdits: Outcome['deleteEdits'];
      inbox: Outcome['inbox'];
    },
  ): ResolvedRow[] => {
    const at = now();
    const base = { module: 'todo', fresh: true, used: null, changes: [] as string[], at };
    const rows: ResolvedRow[] = [];
    for (const v of fresh.values) {
      const [word, icon] = UNIT_WORDS[v.unit] ?? [v.unit, 'other' as ResolvedIcon];
      const title = titleOf(o, v.entity, v.id);
      rows.push({
        ...base,
        id: `pixel-value-${hash(`${v.entity}/${v.id}/${v.unit}/${v.at}/${v.keptDevice}`)}`,
        kind: 'value',
        module: v.entity === 'entry' ? 'journal' : 'todo',
        icon,
        title: `‘${title || v.entity}’ · ${word}`,
        detail: `${show(v.unit, v.kept)} (${deviceName(v.keptDevice)}) kept · ${show(v.unit, v.replaced)} (${deviceName(v.replacedDevice)}) replaced`,
        action:
          v.unit === 'priority' || v.unit === 'when'
            ? `Use ${show(v.unit, v.replaced)} instead`
            : null,
      });
    }
    for (const d of fresh.deleteEdits)
      rows.push({
        ...base,
        id: `pixel-delete-${hash(`${d.entity}/${d.id}/${d.at}`)}`,
        kind: 'delete_edit',
        module: d.entity === 'entry' ? 'journal' : 'todo',
        icon: 'trash',
        title: `‘${titleOf(o, d.entity, d.id) || d.entity}’ · Deleted`,
        detail: `Deleted (${deviceName(d.deleter)}) kept · edited (${deviceName(d.editor)}) kept inside it`,
        action: null,
      });
    for (const i of fresh.inbox)
      rows.push({
        ...base,
        id: `pixel-inbox-${hash(`${i.task}/${i.list}/${i.at}`)}`,
        kind: 'task_inbox',
        icon: 'inbox',
        title: `‘${titleOf(o, 'task', i.task)}’ moved to Inbox`,
        detail: `${titleOf(o, 'list', i.list) || 'A List'} was deleted on ${deviceName(i.deleter)}`,
        action: null,
      });
    return rows;
  };

  /** This device and the phone sync. Returns what it found, in words. */
  function sync() {
    start();
    const at = now();
    // What a person did here since the last sync is written down now, after the phone's edits.
    capture(me, localDocs(), Math.max(at, phone.lastMs + 1));
    const mine = new Set(me.log.map((r) => `${r.stamp.device}/${r.seq}`));
    const theirs = new Set(phone.log.map((r) => `${r.stamp.device}/${r.seq}`));
    const incoming = phone.log.filter((r) => !mine.has(`${r.stamp.device}/${r.seq}`));
    const outgoing = me.log.filter((r) => !theirs.has(`${r.stamp.device}/${r.seq}`));
    syncReplicas(me, phone);
    const o = fold(me.log);

    // Take the merged data in.
    const changedKeys = ports.settings.importDocs(o.docs.filter((d) => d.entity === 'setting'));
    for (const module of new Set(changedKeys.map((k) => k.split('.')[0] as string)))
      ports.announce.settingsChanged(
        module,
        changedKeys
          .filter((k) => k.startsWith(`${module}.`))
          .map((k) => k.slice(module.length + 1)),
      );
    const liveTasks = new Set(
      ports.data.todo
        .exportDocs()
        .filter((d) => d.entity === 'task' && d.values.deleted_at == null)
        .map((d) => d.id),
    );
    const todoChanged = ports.data.todo.importDocs(o.docs);
    // A Task that was showing here and was deleted on the phone: the open sheet is told.
    for (const d of o.docs)
      if (d.entity === 'task' && d.values.deleted_at != null && liveTasks.has(d.id))
        ports.announce.taskDeleted({
          task_id: d.id,
          title: String(d.values.title ?? ''),
          device: PIXEL.name,
        });
    const journalChanged = ports.data.journal.importDocs(o.docs);

    // What is new.
    const fresh = <T>(items: T[], key: (x: T) => string) =>
      items.filter((x) => {
        const k = key(x);
        if (known.has(k)) return false;
        known.add(k);
        return true;
      });
    const docOf = (c: TextClash) => {
      const target = c.entity === 'entry' ? ports.data.journal : ports.data.todo;
      return {
        target,
        doc: target.exportDocs().find((d) => d.entity === c.entity && d.id === c.id),
      };
    };
    /** The text of the clashing field as the app holds it now. */
    const liveText = (c: TextClash) => (): string | undefined => {
      const v = docOf(c).doc?.values[c.field];
      return typeof v === 'string' ? v : undefined;
    };
    const writeText = (c: TextClash) => (text: string) => {
      const field = c.field;
      const { target, doc } = docOf(c);
      if (doc) target.importDocs([{ ...doc, values: { ...doc.values, [field]: text } }]);
      (c.entity === 'entry' ? ports.announce.journalChanged : ports.announce.todoChanged)();
    };
    const conflicts = fresh(
      o.conflicts,
      (c) => `c/${c.entity}/${c.id}/${c.field}/${hash(c.base)}/${hash(c.ours)}/${hash(c.theirs)}`,
    );
    const values = fresh(
      o.values,
      (v) => `v/${v.entity}/${v.id}/${v.unit}/${v.at}/${v.keptDevice}`,
    );
    const deleteEdits = fresh(o.deleteEdits, (d) => `d/${d.entity}/${d.id}/${d.at}`);
    const inbox = fresh(o.inbox, (i) => `i/${i.task}/${i.list}`);
    const merges = fresh(o.mergedLists, (m) => `m/${m.from}/${m.into}`);

    // Show it.
    const events: ActivityEvent[] = [];
    const add = (kind: string, extra: Record<string, string>, line: Partial<ActivityEvent> = {}) =>
      events.push({
        id: `pixel-act-${at}-${++counter}`,
        kind,
        at,
        device: ME_ID,
        peer: PIXEL.id,
        n_in: 0,
        n_out: 0,
        breakdown: null,
        extra,
        ...line,
      });
    if (incoming.length + outgoing.length > 0)
      add(
        'session',
        {},
        {
          n_in: incoming.length,
          n_out: outgoing.length,
          breakdown: breakdown([...incoming, ...outgoing].map((r) => r.entity)),
        },
      );
    for (const c of conflicts) {
      const subject = subjectOf(c, o);
      const words =
        subject.kind === 'entry'
          ? subjectWords('entry', 'text')
          : subject.kind === 'task'
            ? subjectWords('task', subject.field)
            : subjectWords('comment', '');
      const sample = conflictOf(c, o, writeText(c), liveText(c));
      ports.conflicts.raise(sample);
      add('conflict_raised', { subject: words, conflict: sample.row.id });
    }
    for (const m of merges) {
      const [from, into] = [titleOf(o, 'list', m.from), titleOf(o, 'list', m.into)];
      add('lists_merged', { from, into }, { device: PIXEL.id, peer: ME_ID });
      ports.announce.listsMerged({
        from_id: m.from,
        from_name: from,
        into_id: m.into,
        into_name: into,
        device: PIXEL.name,
      });
    }
    const rows = resolvedRows(o, { values, deleteEdits, inbox });
    if (rows.length) ports.conflicts.addResolved(rows);
    if (events.length) {
      ports.activity.add(...events);
      ports.announce.activityChanged();
    }
    if (todoChanged) ports.announce.todoChanged();
    if (journalChanged) ports.announce.journalChanged();
    if (conflicts.length) ports.announce.newConflicts(conflicts.length, PIXEL.name);
    return {
      in: incoming.length,
      out: outgoing.length,
      conflicts: conflicts.length,
      resolved: rows.map((r) => r.kind),
      mergedLists: merges.length,
    };
  }

  // ------------------------------------------------------------------------------ scenarios

  const local = () => {
    const d = new Date();
    const p = (n: number) => String(n).padStart(2, '0');
    return {
      today: `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`,
      now: `${p(d.getHours())}:${p(d.getMinutes())}`,
      offset_min: -d.getTimezoneOffset(),
    };
  };
  const makeTask = (title: string, notes = '', list: string | null = null, today = false) => {
    const task = ports.command('todo_task_create', {
      title,
      listId: list,
      when: today ? { date: local().today, time: null, local: local() } : null,
      details: null,
    }) as { id: string };
    if (notes)
      ports.command('todo_task_update', {
        id: task.id,
        title,
        notes,
        sessionId: `stage-${task.id}`,
      });
    return task.id;
  };
  const taskNamed = (title: string) => {
    const id = ports.data.todo
      .exportDocs()
      .find(
        (d) => d.entity === 'task' && d.values.title === title && d.values.deleted_at == null,
      )?.id;
    return id;
  };
  const listNamed = (name: string) =>
    ports.data.todo
      .exportDocs()
      .find((d) => d.entity === 'list' && d.values.name === name && d.values.deleted_at == null)
      ?.id;

  /** What the phone did and this device has not heard of yet is heard now, before staging more. */
  const flush = () => {
    if (started) sync();
  };

  type Scenario = { about: string; stage(): string; sync: boolean };
  const SCENARIOS: Record<string, Scenario> = {
    // Prototype "Remote change" (H1, H5, F15): Today is open; "Call mum" changed on the phone.
    remote: {
      about: 'Today is open; "Call mum" is changed on the phone',
      sync: true,
      stage() {
        flush();
        // The Task exists here and on the phone before the phone changes it, so the change
        // updates it in place; running this again starts from "Call mum" again.
        const id = taskNamed('Call mum') ?? taskNamed('Call mum at 6');
        if (id) {
          const task = ports.command('todo_task', { id }) as { notes: string };
          ports.command('todo_task_update', {
            id,
            title: 'Call mum',
            notes: task.notes,
            sessionId: `stage-${now()}`,
          });
        }
        const target = id ?? makeTask('Call mum', '', null, true);
        rebase();
        edit('task_edit', { task: target, title: 'Call mum at 6' });
        return 'Open Today first: the title changes under you when this runs.';
      },
    },
    // "Conflicts", "Compare", "Edit merged" (F1 to F8) and "Task sheet" (F9, I15).
    conflict: {
      about: 'A Journal Entry and a Task’s notes edited on both devices',
      sync: true,
      stage() {
        flush();
        const task =
          taskNamed('Plan weekend hike') ??
          makeTask('Plan weekend hike', 'Route: to decide\nBring the stove\n');
        const entry = ports.command('journal_entry_create', {
          entry: {
            page_date: local().today,
            text: 'Went for a run along the canal.\nCoffee with Sam after.\n',
            written_tz: 'UTC',
            written_offset_min: 0,
            session_id: `stage-${now()}`,
          },
        }) as { id: string };
        rebase();
        edit('task_edit', { task, notes: 'Route: lake path from the south\nBring the stove\n' });
        edit('entry_edit', {
          entry: entry.id,
          body: 'Went for a run along the canal, legs heavy but good.\nCoffee with Sam after.\n',
        });
        ports.command('todo_task_update', {
          id: task,
          title: 'Plan weekend hike',
          notes: 'Route: ridge loop from the north\nBring the stove\n',
          sessionId: `stage-b-${now()}`,
        });
        ports.command('journal_entry_update', {
          id: entry.id,
          text: 'Went for a run along the canal, felt great after the rain.\nCoffee with Sam after.\n',
          sessionId: `stage-c-${now()}`,
        });
        return `Task sheet with the Conflict: /todo?task=${task}`;
      },
    },
    // "Deleted elsewhere" (H6): the Task open in the sheet is deleted on the phone. Open it, then sync.
    remoteDelete: {
      about: 'The Task open in the sheet is deleted on the phone',
      sync: false,
      stage() {
        flush();
        const id = taskNamed('Call mum') ?? makeTask('Call mum', 'Ask about Sunday');
        rebase();
        edit('task_delete', { task: id });
        return 'Open the Task "Call mum", then run __pixel.sync().';
      },
    },
    // "List merged" (H7): you are on "Groceries" when it merges with the phone's.
    listMerged: {
      about: 'You are on "Groceries" when it merges with the phone’s "Groceries"',
      sync: false,
      stage() {
        flush();
        rebase();
        // The phone made its "Groceries" first, so it is the older and is kept; the one made here
        // now (the one you open) is the one that goes away. Rust keeps the lower id (ids are
        // time-ordered), hence the leading "-" on the phone's ids: below any uuid made here.
        edit('list_create', { list: 'phone-groceries', name: 'Groceries' });
        edit('task_create', { task: 'phone-eggs', title: 'Eggs', list: 'phone-groceries' });
        const made = Date.now();
        for (let i = 0; i < 1e6 && Date.now() <= made + 1; i++);
        const list = (ports.command('todo_list_create', { name: 'Groceries' }) as { id: string })
          .id;
        makeTask('Oat milk', '', list);
        return 'Open the List "Groceries", then run __pixel.sync(): you are moved to the kept List and told.';
      },
    },
    // "Resolved automatically" (F11, F13): a value clash, a delete that met an edit, a Task put in the Inbox.
    resolved: {
      about: 'Resolved automatically rows: a Priority clash, delete + edit, Task to the Inbox',
      sync: true,
      stage() {
        flush();
        const water = taskNamed('Water plants') ?? makeTask('Water plants');
        const old = taskNamed('Old shopping list') ?? makeTask('Old shopping list', 'Milk');
        const pack = taskNamed('Pack boots') ?? makeTask('Pack boots');
        const trip =
          listNamed('Trip') ??
          (ports.command('todo_list_create', { name: 'Trip' }) as { id: string }).id;
        rebase();
        // This device deletes first, so the phone's edit comes after the delete.
        ports.command('todo_task_delete', { id: old });
        ports.command('todo_list_delete', { id: trip, choice: { kind: 'move_to_inbox' } });
        capture(me, localDocs(), now());
        edit('task_priority', { task: water, priority: 1 });
        edit('task_edit', { task: old, notes: 'Milk, oat milk' });
        edit('task_move', { task: pack, list: trip });
        ports.command('todo_task_set_priority', { id: water, priority: 2 });
        return 'See Sync › Conflicts › Resolved automatically.';
      },
    },
  };

  return {
    edit,
    sync,
    /** What the phone shows now, as docs. */
    world: () => fold(phone.log).docs,
    scenarios: () => Object.fromEntries(Object.entries(SCENARIOS).map(([k, s]) => [k, s.about])),
    scenario(name: string) {
      const s = SCENARIOS[name];
      if (!s) throw new Error(`unknown scenario ${name}: ${Object.keys(SCENARIOS).join(', ')}`);
      const hint = s.stage();
      // Staging writes through the mock's commands, which tell no one: the lists, Tasks and Entries on show
      // are read again, as the real path does after a change.
      ports.announce.todoChanged();
      ports.announce.journalChanged();
      const result = s.sync ? sync() : null;
      return { hint, result };
    },
  };
}
