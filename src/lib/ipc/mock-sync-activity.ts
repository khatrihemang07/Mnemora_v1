/**
 * Activity for the browser mock: a long synced log (thousands of lines, default 4000, `?activity=N`
 * for another size) so scrolling and paging can be judged by eye. It mirrors crates/sync/src/
 * activity_view.rs: newest first, cursor `at:id`, the same sentences, filters by device, and a
 * session logged by the other device reading the other way round. Made once, from a fixed seed.
 */
import type { ActivityFilter, ActivityPage, ActivityRow, AppError, Piece } from './bindings';

const COMMANDS = ['sync_activity'];

const ME = 'mock-me';
const ME_NAME = 'Alex’s MacBook Pro';
const PIXEL = { id: 'mock-pixel', name: 'Pixel 8 (simulated)' };
const AIR = { id: 'mock-air', name: 'MacBook Air (simulated)' };
const DEVICES = [{ id: ME, name: ME_NAME }, PIXEL, AIR];
const NAMES = new Map(DEVICES.map((d) => [d.id, d.name]));

const MINUTE = 60_000;

/** An event as the log holds it, before it is put in words for a viewer. */
export interface Event {
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

/** How the Activity line names the text a Conflict is about: "a Task’s notes", "an Entry’s text". */
export function subjectWords(entity: string, field: string): string {
  if (entity === 'entry') return 'an Entry’s text';
  if (entity === 'task') return `a Task’s ${field}`;
  return 'a Comment’s text';
}

let written = 0;
/**
 * A line this device writes now (the mock's counterpart of Rust's `blank` row): by this device
 * unless `device` says another made it, about `peer` when there is one.
 */
export function lineNow(
  kind: string,
  at: number,
  rest: { device?: string; peer?: string; extra?: Record<string, string> } = {},
): Event {
  return {
    id: `mock-act-now-${at}-${String(++written).padStart(4, '0')}`,
    kind,
    at,
    device: rest.device ?? ME,
    peer: rest.peer ?? '',
    n_in: 0,
    n_out: 0,
    breakdown: null,
    extra: rest.extra ?? {},
  };
}

/** A small seeded generator, so the same log is made every time. */
function seeded(seed: number) {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1_664_525) + 1_013_904_223) >>> 0;
    return s / 2 ** 32;
  };
}

/** The kinds of record a session moves, by Module, as the Rust breakdown words them. */
const KINDS = [
  { module: 'Todo', one: 'Task', many: 'Tasks' },
  { module: 'Todo', one: 'List', many: 'Lists' },
  { module: 'Todo', one: 'Comment', many: 'Comments' },
  { module: 'Journal', one: 'Entry', many: 'Entries' },
  { module: 'Settings', one: '', many: '' },
];

/**
 * "Todo: 9 Tasks, 1 List · Journal: 2 Entries · Settings: 1" for `total` records: the counts always
 * add up to the in + out of the line, as Rust's breakdown (records moved, both directions) does.
 */
export function breakdownOf(total: number, rand: () => number): string {
  const kinds = KINDS.filter(() => rand() < 0.5);
  if (kinds.length === 0) kinds.push(KINDS[0] as (typeof KINDS)[number]);
  const used = kinds.slice(0, Math.max(1, Math.min(kinds.length, total)));
  const counts = used.map(() => 1);
  for (let left = total - used.length; left > 0; left--) {
    const i = Math.floor(rand() * used.length);
    counts[i] = (counts[i] ?? 0) + 1;
  }
  const modules: string[] = [];
  for (const [i, kind] of used.entries()) {
    const n = counts[i] ?? 0;
    const word = kind.module === 'Settings' ? `${n}` : `${n} ${n === 1 ? kind.one : kind.many}`;
    const at = modules.findIndex((m) => m.startsWith(`${kind.module}: `));
    if (at < 0) modules.push(`${kind.module}: ${word}`);
    else modules[at] = `${modules[at]}, ${word}`;
  }
  return modules.join(' · ');
}

/** `count` events going back from `now`, newest last in the array (sorted by the caller). */
export function makeEvents(count: number, now: number): Event[] {
  const rand = seeded(68);
  const pick = <T>(xs: T[]): T => xs[Math.floor(rand() * xs.length)] as T;
  const events: Event[] = [];
  let at = now - 2 * MINUTE;
  const waiting = [
    { id: 'mock-conflict-notes', subject: subjectWords('task', 'notes') },
    { id: 'mock-conflict-entry', subject: subjectWords('entry', 'text') },
  ];
  for (let i = 0; i < count; i++) {
    at -= (4 + Math.floor(rand() * 70)) * MINUTE;
    const r = rand();
    const device = r < 0.45 ? ME : pick([PIXEL.id, AIR.id]);
    const others = DEVICES.filter((d) => d.id !== device);
    const peer = pick(others).id;
    const base = { id: `mock-act-${String(i).padStart(6, '0')}`, at, device, peer };
    const none = { n_in: 0, n_out: 0, breakdown: null, extra: {} };
    if (r < 0.8) {
      const n_in = 1 + Math.floor(rand() * 40);
      const n_out = Math.floor(rand() * 8);
      events.push({
        ...base,
        kind: 'session',
        n_in,
        n_out,
        breakdown: breakdownOf(n_in + n_out, rand),
        extra: {},
      });
    } else if (r < 0.86) {
      // Only the newest two still wait (the mock's two sample Conflicts), each worded for its own
      // text; the rest were settled since.
      const open = at > now - 24 * 60 * MINUTE ? waiting.shift() : undefined;
      events.push({
        ...base,
        ...none,
        kind: 'conflict_raised',
        extra: open
          ? { subject: open.subject, conflict: open.id }
          : { subject: subjectWords('task', 'notes') },
      });
    } else if (r < 0.9) {
      events.push({
        ...base,
        ...none,
        kind: 'conflict_resolved',
        extra: { subject: 'an Entry’s text' },
      });
    } else if (r < 0.93) {
      events.push({
        ...base,
        ...none,
        kind: 'lists_merged',
        extra: { from: 'Groceries', into: 'Groceries' },
      });
    } else if (r < 0.95) {
      events.push({
        ...base,
        ...none,
        kind: 'paused',
        extra: { why: 'one of you needs an update' },
      });
    } else if (r < 0.97) {
      events.push({ ...base, ...none, kind: 'resumed' });
    } else if (r < 0.99) {
      events.push({ ...base, ...none, kind: 'failed', extra: { why: 'lost' } });
    } else {
      events.push({ ...base, ...none, kind: 'renamed', extra: { old: 'Air', new: AIR.name } });
    }
  }
  // The oldest line of all: the first pairing.
  events.push({
    id: 'mock-act-paired',
    kind: 'paired',
    at: at - 60 * MINUTE,
    device: ME,
    peer: PIXEL.id,
    n_in: 0,
    n_out: 0,
    breakdown: null,
    extra: {},
  });
  return events;
}

const piece = (text: string, strong = false): Piece => ({ text, strong });

/** The event as `viewer` reads it (the Rust `describe`, in short). */
export function describe(
  e: Event,
  viewer: string,
  isOpen: (conflict: string) => boolean = () => false,
): ActivityRow {
  const name = (id: string) =>
    NAMES.get(id) ?? (id === e.peer ? e.extra.peer_name : undefined) ?? 'another device';
  const aboutMe = e.peer === viewer;
  const other = name(aboutMe ? e.device : e.peer);
  let text: Piece[];
  switch (e.kind) {
    case 'session': {
      const [i, o] = aboutMe ? [e.n_out, e.n_in] : [e.n_in, e.n_out];
      text = [piece('Synced with '), piece(other, true), piece(` · ${i} in, ${o} out`)];
      break;
    }
    case 'paused':
      text = [piece('Paused with '), piece(other, true), piece(`: ${e.extra.why}`)];
      break;
    case 'resumed':
      text = [piece('Resumed with '), piece(other, true)];
      break;
    case 'failed':
      text = [piece('Connection to '), piece(other, true), piece(' lost; retrying')];
      break;
    case 'paired':
      text = [piece('Paired with '), piece(other, true)];
      break;
    case 'forgot':
      text = [piece('Forgot '), piece(name(e.peer), true)];
      break;
    case 'renamed':
      text = [
        piece(e.extra.old ?? '', true),
        piece(' renamed to '),
        piece(e.extra.new ?? '', true),
      ];
      break;
    case 'left':
      // Plain on the device that left; the others name it (Rust: activity_view.rs).
      text =
        e.device === viewer
          ? [piece('Left the group')]
          : [piece(name(e.device), true), piece(' left the group')];
      break;
    case 'conflict_raised':
      text = [piece(`Conflict in ${e.extra.subject} needs you`)];
      break;
    case 'conflict_resolved':
      text = [piece(`Conflict in ${e.extra.subject} resolved`)];
      break;
    default:
      text = [
        piece(e.extra.from ?? '', true),
        piece(' ('),
        piece(other, true),
        piece(') merged into '),
        piece(e.extra.into ?? '', true),
      ];
  }
  const named = e.kind === 'renamed' || e.kind === 'left' || e.kind === 'forgot';
  return {
    id: e.id,
    kind: e.kind,
    text,
    on: e.device !== viewer && !aboutMe && !named ? name(e.device) : null,
    at: e.at,
    breakdown: e.breakdown,
    // Like Rust: the link is there while the Conflict still waits, whatever the line said when written.
    link:
      e.kind === 'conflict_raised' && e.extra.conflict && isOpen(e.extra.conflict)
        ? 'conflicts'
        : null,
  };
}

function involves(e: Event, filter: ActivityFilter, viewer: string): boolean {
  if (filter.kind === 'all') return true;
  const id = filter.kind === 'this_device' ? viewer : filter.id;
  return e.device === id || e.peer === id;
}

const invalid = (message: string): AppError => ({ code: 'validation', message, details: null });

/** Sorted newest first, ties by id descending (the Rust order). */
const newestFirst = (a: Event, b: Event) => b.at - a.at || (a.id < b.id ? 1 : a.id > b.id ? -1 : 0);

export function createActivityMock(
  count = 4000,
  now = Date.now(),
  isOpen: (conflict: string) => boolean = () => true,
) {
  const events = makeEvents(count, now).sort(newestFirst);
  return {
    /** Lines a sync wrote (the simulated device's), in with the rest. */
    add(...added: Event[]) {
      events.push(...added);
      events.sort(newestFirst);
    },
    handles: (cmd: string) => COMMANDS.includes(cmd),
    handle(cmd: string, a: Record<string, unknown>): unknown {
      if (cmd !== 'sync_activity') return undefined;
      const filter = a.filter as ActivityFilter;
      const cursor = a.cursor as string | null;
      const limit = Math.min(200, Math.max(1, Number(a.limit) || 1));
      let start = 0;
      if (cursor) {
        const split = cursor.indexOf(':');
        if (split < 0) throw invalid('That page is gone');
        const at = Number(cursor.slice(0, split));
        const id = cursor.slice(split + 1);
        // After the cursor: older, or the same moment with a lower id.
        start = events.findIndex((e) => e.at < at || (e.at === at && e.id < id));
        if (start < 0) start = events.length;
      }
      const matching: Event[] = [];
      for (let i = start; i < events.length && matching.length <= limit; i++) {
        const e = events[i] as Event;
        if (involves(e, filter, ME)) matching.push(e);
      }
      const page = matching.slice(0, limit);
      const last = page.at(-1);
      return {
        rows: page.map((e) => describe(e, ME, isOpen)),
        next: matching.length > limit && last ? `${last.at}:${last.id}` : null,
      } satisfies ActivityPage;
    },
  };
}
