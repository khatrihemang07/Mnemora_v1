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
interface Event {
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

/** A small seeded generator, so the same log is made every time. */
function seeded(seed: number) {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1_664_525) + 1_013_904_223) >>> 0;
    return s / 2 ** 32;
  };
}

const BREAKDOWNS = [
  'Todo: 9 Tasks, 1 List · Journal: 2 Entries · Settings: 1',
  'Todo: 3 Tasks',
  'Journal: 1 Entry',
  'Todo: 14 Tasks, 2 Lists, 5 Comments',
  'Settings: 2',
  'Todo: 1 Task · Journal: 1 Entry',
];

/** `count` events going back from `now`, newest last in the array (sorted by the caller). */
export function makeEvents(count: number, now: number): Event[] {
  const rand = seeded(68);
  const pick = <T>(xs: T[]): T => xs[Math.floor(rand() * xs.length)] as T;
  const events: Event[] = [];
  let at = now - 2 * MINUTE;
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
        breakdown: pick(BREAKDOWNS),
        extra: {},
      });
    } else if (r < 0.86) {
      events.push({
        ...base,
        ...none,
        kind: 'conflict_raised',
        extra: { subject: 'a Task’s notes' },
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
export function describe(e: Event, viewer: string): ActivityRow {
  const name = (id: string) => NAMES.get(id) ?? 'another device';
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
    case 'renamed':
      text = [
        piece(e.extra.old ?? '', true),
        piece(' renamed to '),
        piece(e.extra.new ?? '', true),
      ];
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
        piece(' from '),
        piece(other, true),
        piece(' merged into '),
        piece(e.extra.into ?? '', true),
      ];
  }
  const named = e.kind === 'renamed';
  return {
    id: e.id,
    kind: e.kind,
    text,
    on: e.device !== viewer && !aboutMe && !named ? name(e.device) : null,
    at: e.at,
    breakdown: e.breakdown,
    link: e.kind === 'conflict_raised' ? 'conflicts' : null,
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

export function createActivityMock(count = 4000, now = Date.now()) {
  const events = makeEvents(count, now).sort(newestFirst);
  return {
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
        rows: page.map((e) => describe(e, ME)),
        next: matching.length > limit && last ? `${last.at}:${last.id}` : null,
      } satisfies ActivityPage;
    },
  };
}
