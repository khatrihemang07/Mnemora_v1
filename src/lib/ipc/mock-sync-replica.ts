/**
 * One simulated device for the merge: a log of records (mock-sync-merge.ts folds it into data),
 * the edits a person makes (the fixture's ops, which the Pixel's console helpers use too), and the
 * sync of two devices (both end up with the union of their logs).
 */

import { nextDue, type RepeatRule } from '../ui/when-repeat';
import {
  type Doc,
  type Entity,
  fold,
  type Json,
  LOOK_KEYS,
  type Outcome,
  type Rec,
  same,
} from './mock-sync-merge';
import { keyAfter, keyBetween } from './mock-todo-order';

/** What today is on both devices in the fixture (src-tauri/src/sync_merge_fixture_tests.rs `local`). */
const TODAY = '2026-10-22';

/** The id of the occurrence copy of a Task due on a day: the same on every device (Rust `occurrence_id`). */
const occurrenceId = (task: string, due: string) => `occurrence|${task}|${due}`;

export interface Replica {
  device: string;
  log: Rec[];
  seq: number;
  /** Look and feel: kept here, never sent. */
  look: Record<string, Json>;
  lastMs: number;
  lastCounter: number;
}

export function createReplica(device: string): Replica {
  return { device, log: [], seq: 0, look: {}, lastMs: 0, lastCounter: 0 };
}

/** Both devices hold every record of both. */
export function syncReplicas(a: Replica, b: Replica) {
  const seen = new Set<string>();
  const union: Rec[] = [];
  for (const r of [...a.log, ...b.log]) {
    const key = `${r.stamp.device}/${r.seq}`;
    if (seen.has(key)) continue;
    seen.add(key);
    union.push(r);
  }
  const ms = Math.max(a.lastMs, b.lastMs);
  for (const r of [a, b]) {
    r.log = [...union];
    r.lastMs = ms;
  }
}

const find = (o: Outcome, entity: Entity, id: string): Doc | undefined =>
  o.docs.find((d) => d.entity === entity && d.id === id);

/** Writes one change: `values` that differ from `shown` (what this device shows), with the group completed. */
function append(
  r: Replica,
  entity: Entity,
  id: string,
  values: Record<string, Json>,
  now: number,
  shown: Record<string, Json>,
) {
  const fields: Record<string, Json> = {};
  const changed: string[] = [];
  for (const [k, value] of Object.entries(values)) {
    if (k in shown && same(shown[k], value)) continue;
    changed.push(k);
    for (const m of GROUPS[k] ?? [k]) fields[m] = m === k ? value : (values[m] ?? shown[m] ?? null);
  }
  if (Object.keys(fields).length === 0) return;
  // A touched group travels complete, but `prev` holds only what this edit changed (Rust
  // change_log.prev), so a clash needs both sides to have changed the same member.
  const prev = Object.fromEntries(
    changed.flatMap((k) => (k in shown ? [[k, shown[k] as Json]] : [])),
  );
  const ms = Math.max(now, r.lastMs);
  const counter = ms === r.lastMs ? r.lastCounter + 1 : 0;
  r.lastMs = ms;
  r.lastCounter = counter;
  const rec: Rec = {
    entity,
    id,
    stamp: { ms, counter, device: r.device },
    seq: ++r.seq,
    fields,
    prev,
  };
  if (fields.deleted_at != null) {
    rec.seen = {};
    for (const o of r.log)
      if (o.entity === entity && o.stamp.device !== r.device)
        rec.seen[o.stamp.device] = Math.max(rec.seen[o.stamp.device] ?? 0, o.seq);
  }
  r.log.push(rec);
}

function write(r: Replica, entity: Entity, id: string, values: Record<string, Json>, now: number) {
  append(r, entity, id, values, now, find(fold(r.log), entity, id)?.values ?? {});
}

/** Writes what a person did outside the merge (in the app): every doc whose values differ from what the log shows. */
export function capture(r: Replica, docs: Doc[], now: number) {
  const shown = new Map(fold(r.log).docs.map((d) => [`${d.entity}/${d.id}`, d.values]));
  for (const d of docs)
    append(r, d.entity, d.id, d.values, now, shown.get(`${d.entity}/${d.id}`) ?? {});
}

const WHEN = ['due_date', 'due_time', 'repeat_freq', 'repeat_every', 'repeat_days', 'repeat_mode'];
const GROUPS: Record<string, string[]> = {
  title: ['title', 'understood'],
  understood: ['title', 'understood'],
  ...Object.fromEntries(WHEN.map((k) => [k, WHEN])),
  state: ['state', 'closed_at'],
  closed_at: ['state', 'closed_at'],
  body: ['body', 'edited'],
  edited: ['body', 'edited'],
};

export interface Step {
  d?: string;
  op: string;
  [key: string]: unknown;
}

/** Where ids and labels come from: a fixture step names things by label. */
export interface Ids {
  labels: Map<string, string>;
  next(kind: string): string;
}

export function createIds(prefix = ''): Ids {
  let n = 0;
  return {
    labels: new Map(),
    next: (kind) => `${prefix}${kind}-${String(++n).padStart(6, '0')}`,
  };
}

const live = (o: Outcome, entity: Entity) =>
  o.docs.filter((d) => d.entity === entity && d.values.deleted_at == null);
const key = (d: Doc) => String(d.values.sort_key);
const byKey = (a: Doc, b: Doc) =>
  key(a) < key(b) ? -1 : key(a) > key(b) ? 1 : a.id < b.id ? -1 : 1;
const lastOf = (docs: Doc[]) =>
  docs.map(key).reduce<string | null>((m, k) => (m === null || k > m ? k : m), null);
const endKey = (o: Outcome, list: Json) =>
  keyAfter(lastOf(live(o, 'task').filter((t) => t.values.list_id === list)));

/** One edit by a person on this device. Mirrors the commands the fixture drives in Rust. */
export function perform(r: Replica, s: Step, ids: Ids, now: number) {
  const o = fold(r.log);
  const ref = (name: string): string | null => {
    const label = s[name];
    return typeof label === 'string' ? (ids.labels.get(label) ?? label) : null;
  };
  const set = (entity: Entity, id: string, values: Record<string, Json>) =>
    write(r, entity, id, values, now);
  const made = (name: string, kind: string) => {
    const id = ids.next(kind);
    ids.labels.set(String(s[name]), id);
    return id;
  };
  const task = ref('task') as string;
  switch (s.op) {
    case 'list_create': {
      const id = made('list', 'list');
      set('list', id, {
        name: String(s.name),
        emoji: null,
        task_sort: 'manual',
        group_by: 'none',
        sort_key: keyAfter(lastOf(live(o, 'list'))),
        created_at: now,
        deleted_at: null,
      });
      return id;
    }
    case 'list_rename':
      return set('list', ref('list') as string, { name: String(s.name) });
    case 'list_sort':
      return set('list', ref('list') as string, { task_sort: String(s.sort) });
    case 'list_group':
      return set('list', ref('list') as string, { group_by: String(s.group) });
    case 'list_delete': {
      const id = ref('list') as string;
      for (const t of live(o, 'task').filter((t) => t.values.list_id === id))
        set('task', t.id, { list_id: null, sort_key: endKey(o, null) });
      return set('list', id, { deleted_at: now });
    }
    case 'task_create': {
      const id = made('task', 'task');
      const list = ref('list');
      set('task', id, {
        list_id: list,
        title: String(s.title),
        understood: (s.understood ?? null) as Json,
        notes: String(s.notes ?? ''),
        due_date: null,
        due_time: null,
        repeat_freq: null,
        repeat_every: null,
        repeat_days: null,
        repeat_mode: null,
        priority: 4,
        state: 'open',
        closed_at: null,
        repeat_of: null,
        sort_key: endKey(o, list),
        created_at: now,
        deleted_at: null,
      });
      return id;
    }
    case 'task_edit':
      return set('task', task, {
        ...('title' in s ? { title: String(s.title) } : {}),
        ...('notes' in s ? { notes: String(s.notes) } : {}),
      });
    case 'task_priority':
      return set('task', task, { priority: Number(s.priority) });
    case 'task_title':
      return set('task', task, {
        title: String(s.title),
        understood: (s.understood ?? null) as Json,
      });
    case 'task_when': {
      const rule = (s.repeat ?? null) as RepeatRule | null;
      return set('task', task, {
        due_date: (s.date ?? null) as Json,
        due_time: (s.time ?? null) as Json,
        ...repeatFields(rule),
      });
    }
    case 'task_skip': {
      const v = find(o, 'task', task)?.values ?? {};
      const rule = ruleOf(v);
      if (!rule) throw new Error('Only a repeating Task can be skipped');
      const due = (v.due_date as string | null) ?? TODAY;
      return set('task', task, { due_date: nextDue(rule, due, TODAY), skipped: true });
    }
    case 'task_done': {
      const v = find(o, 'task', task)?.values ?? {};
      const rule = ruleOf(v);
      if (!rule) return set('task', task, { state: 'done', closed_at: now });
      // A repeating Task is not closed: this occurrence is recorded as a done copy (one id for
      // everyone, so completing it on two devices is one record) and the Task moves on.
      const due = (v.due_date as string | null) ?? TODAY;
      set('task', occurrenceId(task, due), {
        list_id: v.list_id ?? null,
        title: v.title ?? '',
        understood: null,
        notes: v.notes ?? '',
        due_date: v.due_date ?? null,
        due_time: v.due_time ?? null,
        ...repeatFields(null),
        priority: v.priority ?? 4,
        state: 'done',
        closed_at: now,
        repeat_of: task,
        sort_key: v.sort_key ?? '',
        created_at: now,
        deleted_at: null,
      });
      return set('task', task, { due_date: nextDue(rule, due, TODAY) });
    }
    case 'task_move':
      return set('task', task, { list_id: ref('list'), sort_key: endKey(o, ref('list')) });
    case 'task_delete':
      return set('task', task, { deleted_at: now });
    case 'task_place': {
      const list = find(o, 'task', task)?.values.list_id ?? null;
      const others = live(o, 'task')
        .filter((t) => t.values.list_id === list && t.values.state === 'open' && t.id !== task)
        .sort(byKey);
      const target = ref('before');
      const at = others.findIndex((t) => t.id === target);
      const next = others[at] as Doc | undefined;
      const above = others[at - 1] as Doc | undefined;
      const sort_key = next
        ? keyBetween(above ? key(above) : null, key(next))
        : keyAfter(lastOf(others));
      return set('task', task, { sort_key });
    }
    case 'comment_add': {
      const id = made('comment', 'comment');
      return set('comment', id, {
        body: String(s.body),
        edited: false,
        task_id: task,
        created_at: now,
        deleted_at: null,
      });
    }
    case 'comment_edit':
      return set('comment', ref('comment') as string, { body: String(s.body), edited: true });
    case 'entry_create': {
      const id = made('entry', 'entry');
      return set('entry', id, {
        body: String(s.body),
        page_date: String(s.page_date ?? '2026-10-21'),
        written_at: now,
        written_offset_min: 0,
        written_tz: 'UTC',
        created_at: now,
        deleted_at: null,
      });
    }
    case 'entry_delete':
      return set('entry', ref('entry') as string, { deleted_at: now });
    case 'entry_edit':
      return set('entry', ref('entry') as string, { body: String(s.body) });
    case 'pref': {
      const k = String(s.key);
      if (LOOK_KEYS.includes(k)) r.look[k] = s.value as Json;
      else set('setting', k, { value: s.value as Json });
      return;
    }
    default:
      throw new Error(`unknown op ${String(s.op)}`);
  }
}

/** The Preferences this device shows: `defaults`, then the synced ones, then its own look and feel. */
export function settingsOf(r: Replica, defaults: Record<string, Json> = {}): Record<string, Json> {
  const out = { ...defaults };
  for (const d of fold(r.log).docs) if (d.entity === 'setting') out[d.id] = d.values.value as Json;
  return { ...out, ...r.look };
}

/** The kinds of Resolved automatically a device lists, sorted (what the fixture compares). */
export function resolvedKinds(o: Outcome): string[] {
  return [
    ...o.values.map(() => 'value'),
    ...o.deleteEdits.map(() => 'delete_edit'),
    ...o.inbox.map(() => 'task_inbox'),
    ...o.deletedConflicts.map(() => 'conflict_deleted'),
  ].sort();
}

/** A Task's Repeat as its columns (none clears them). */
function repeatFields(rule: RepeatRule | null): Record<string, Json> {
  return {
    repeat_freq: rule?.freq ?? null,
    repeat_every: rule?.every ?? null,
    repeat_days: rule?.days ?? null,
    repeat_mode: rule?.mode ?? null,
  };
}

/** The Repeat a Task's values hold, or none. */
function ruleOf(v: Record<string, Json>): RepeatRule | null {
  if (v.repeat_freq == null || v.repeat_every == null || v.repeat_mode == null) return null;
  return {
    freq: v.repeat_freq as RepeatRule['freq'],
    every: Number(v.repeat_every),
    days: (v.repeat_days ?? null) as number | null,
    mode: v.repeat_mode as RepeatRule['mode'],
  };
}

/** The data as the fixture's `expect` words it. */
export function worldOf(r: Replica, ids: Ids, defaults: Record<string, Json> = {}) {
  const o = fold(r.log);
  const label = (id: Json): Json => {
    if (id === null) return null;
    for (const [l, v] of ids.labels) if (v === id) return l;
    return '?';
  };
  const lists = live(o, 'list');
  const order = (list: string | null) =>
    live(o, 'task')
      .filter((t) => t.values.list_id === list && t.values.state === 'open')
      .sort(byKey)
      .map((t) => label(t.id));
  return {
    lists: Object.fromEntries(lists.map((l) => [label(l.id), l.values.name])),
    tasks: Object.fromEntries(
      o.docs
        .filter((d) => d.entity === 'task')
        .map((t) => {
          const deleted = t.values.deleted_at != null;
          const v = t.values;
          return [
            label(t.id),
            {
              title: v.title,
              understood: v.understood ?? null,
              notes: v.notes,
              priority: v.priority,
              due_date: v.due_date,
              due_time: v.due_time,
              repeat_freq: v.repeat_freq ?? null,
              repeat_every: v.repeat_every ?? null,
              repeat_days: v.repeat_days ?? null,
              repeat_mode: v.repeat_mode ?? null,
              state: v.state,
              deleted,
              list: deleted ? null : label(v.list_id as Json),
            },
          ];
        }),
    ),
    arrange: Object.fromEntries(
      lists.map((l) => [
        label(l.id),
        { sort: l.values.task_sort ?? 'manual', group: l.values.group_by ?? 'none' },
      ]),
    ),
    occurrences: live(o, 'task').filter((t) => t.values.repeat_of != null).length,
    order: {
      inbox: order(null),
      ...Object.fromEntries(lists.map((l) => [label(l.id), order(l.id)])),
    },
    comments: Object.fromEntries(
      live(o, 'comment').map((c) => [
        label(c.id),
        {
          task: label(c.values.task_id as Json),
          body: c.values.body,
          edited: c.values.edited === true,
        },
      ]),
    ),
    entries: Object.fromEntries(live(o, 'entry').map((e) => [label(e.id), e.values.body])),
    settings: settingsOf(r, defaults),
    conflicts: o.conflicts.length,
    resolved: resolvedKinds(o),
  };
}
