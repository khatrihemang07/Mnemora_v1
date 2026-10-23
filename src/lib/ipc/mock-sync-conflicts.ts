/**
 * Conflicts for the browser mock: two sample clashes (a Journal Entry and a Task's notes) with a
 * simulated "Pixel 8", and the same shapes Rust sends (crates/sync/src/conflict_view.rs). The
 * merge itself is Rust's; here each sample carries its clashes ready made.
 */

import type {
  AppError,
  ClashChoice,
  ClashPiece,
  ConflictChoice,
  ConflictList,
  ConflictRow,
  ConflictView,
  DiffSpan,
  ResolvedRow,
} from './bindings';
import { type Piece, textMerge } from './mock-sync-merge';

export interface Sample {
  row: ConflictRow;
  base: string;
  showing: string;
  other: string;
  open: boolean;
  resolution: string | null;
  /** The text of the field as the app holds it now; absent for the ready-made samples. */
  live?: () => string | undefined;
  /** The simulated device's Conflicts: what to write into the data when a person chooses the other text. */
  apply?: (text: string) => void;
}

const COMMANDS = [
  'sync_conflicts',
  'sync_conflict',
  'sync_conflict_draft',
  'sync_conflict_resolve',
  'sync_resolution_use_other',
  'sync_resolutions_visited',
];

/**
 * A few "Resolved automatically" rows, worded as Rust words them (crates/sync/src/resolved_view.rs):
 * a Priority clash and a Due date clash with their buttons, a delete that met an edit, a Task put in
 * the Inbox, and a session's folded Order & preferences line. The first two are new since the last
 * visit. `at` is minutes past the day's start so the rows group under Today.
 */
function resolvedSamples(now: number): ResolvedRow[] {
  const day = new Date(now);
  const at = (h: number, m: number) =>
    new Date(day.getFullYear(), day.getMonth(), day.getDate(), h, m).getTime();
  const base = { module: 'todo', action: null, used: null, changes: [] as string[] };
  return [
    {
      ...base,
      id: 'mock-resolved-priority',
      kind: 'value',
      icon: 'flag',
      title: '‘Water plants’ · Priority',
      detail: 'P1 (Alex’s MacBook Pro) kept · P2 (Pixel 8 (simulated)) replaced',
      at: at(9, 31),
      fresh: true,
      action: 'Use P2 instead',
    },
    {
      ...base,
      id: 'mock-resolved-due',
      kind: 'value',
      icon: 'calendar',
      title: '‘Call bank’ · Due date',
      detail: 'Fri 23 Oct (Pixel 8 (simulated)) kept · Thu 22 Oct (Alex’s MacBook Pro) replaced',
      at: at(9, 20),
      fresh: true,
      action: 'Use Thu 22 Oct instead',
    },
    {
      ...base,
      id: 'mock-resolved-delete',
      kind: 'delete_edit',
      icon: 'trash',
      title: '‘Old shopping list’ · Deleted',
      detail:
        'Deleted (Pixel 8 (simulated)) kept · notes edited (Alex’s MacBook Pro) kept inside it',
      at: at(8, 50),
      fresh: false,
    },
    {
      ...base,
      id: 'mock-resolved-inbox',
      kind: 'task_inbox',
      icon: 'inbox',
      title: '‘Pack boots’ · List',
      detail: 'Moved to the Inbox · ‘Trip’ is gone · Alex’s MacBook Pro had moved it there',
      at: at(8, 40),
      fresh: false,
    },
    {
      ...base,
      id: 'mock-resolved-fold',
      kind: 'order_prefs',
      icon: 'order',
      module: 'sync',
      title: 'Order & preferences · 4 changes',
      detail: 'Pixel 8 (simulated)',
      at: at(8, 31),
      fresh: false,
      changes: ['‘Call mum’ moved', '‘Buy oat milk’ moved', 'Week start', 'Time format'],
    },
  ];
}

/** Words, runs of spaces and line breaks, as Rust cuts them. */
function tokens(text: string): string[] {
  return text.match(/\n|[^\S\n]+|[^\s]+/g) ?? [];
}

/** The words each text has that the other lacks (a longest-common-subsequence diff). */
export function diffSpans(a: string, b: string): [DiffSpan[], DiffSpan[]] {
  const ta = tokens(a);
  const tb = tokens(b);
  const w = tb.length + 1;
  const lcs = new Array<number>((ta.length + 1) * w).fill(0);
  const get = (i: number, j: number) => lcs[i * w + j] ?? 0;
  for (let i = ta.length - 1; i >= 0; i--) {
    for (let j = tb.length - 1; j >= 0; j--) {
      lcs[i * w + j] =
        ta[i] === tb[j] ? get(i + 1, j + 1) + 1 : Math.max(get(i + 1, j), get(i, j + 1));
    }
  }
  const ca = new Array<boolean>(ta.length).fill(true);
  const cb = new Array<boolean>(tb.length).fill(true);
  let i = 0;
  let j = 0;
  while (i < ta.length && j < tb.length) {
    if (ta[i] === tb[j]) {
      ca[i++] = false;
      cb[j++] = false;
    } else if (get(i + 1, j) >= get(i, j + 1)) i++;
    else j++;
  }
  const bridge = (t: string[], c: boolean[]) => {
    for (let k = 1; k < t.length - 1; k++)
      if (!c[k] && c[k - 1] && c[k + 1] && !/\S/.test(t[k] ?? '')) c[k] = true;
  };
  bridge(ta, ca);
  bridge(tb, cb);
  const spans = (t: string[], c: boolean[]) => {
    const out: DiffSpan[] = [];
    t.forEach((w, k) => {
      const last = out[out.length - 1];
      if (last && last.changed === c[k]) last.text += w;
      else out.push({ text: w, changed: c[k] === true });
    });
    return out;
  };
  return [spans(ta, ca), spans(tb, cb)];
}

/** The text showing now: the record's live text, or the one the Conflict was raised with. */
const showingNow = (s: Sample): string => s.live?.() ?? s.showing;

/**
 * The clashes of the other version against what shows now, as `pieces` in conflict_view.rs: the
 * older version is "ours", the showing text "theirs", so an edit made since the Conflict was
 * raised stays (Edit merged).
 */
function piecesOf(s: Sample): ClashPiece[] {
  return textMerge(s.base, s.other, showingNow(s)).map((p: Piece) =>
    'agreed' in p
      ? { kind: 'agreed', text: p.agreed }
      : { kind: 'clash', showing: p.clash.theirs, other: p.clash.ours, original: p.clash.base },
  );
}

/** Mirrors `draft` in conflict_view.rs. */
function draftOf(pieces: ClashPiece[], choices: ClashChoice[]): string {
  let n = 0;
  return pieces
    .map((p) => {
      if (p.kind === 'agreed') return p.text;
      const choice = choices[n++] ?? 'showing';
      if (choice === 'showing') return p.showing;
      if (choice === 'other') return p.other;
      return `${p.showing} ${p.other}`;
    })
    .join('');
}

const at = (h: number, m: number) => new Date(2026, 9, 21, h, m).getTime();

/**
 * `paired` says whether any device is paired: the ready-made samples (two Conflicts, five Resolved
 * rows) only exist with one. What a sync of the simulated device adds is always listed.
 */
export function createConflictsMock(
  emit: () => void = () => {},
  now: () => number = Date.now,
  paired: () => boolean = () => true,
) {
  const resolved = resolvedSamples(now());
  const sampleIds = new Set(resolved.map((r) => r.id));
  const entryShowing =
    'Went for a run along the canal, felt great after the rain.\nCoffee with Sam after.\n';
  const entryOther =
    'Went for a run along the canal, legs heavy but good.\nCoffee with Sam after.\n';
  const notesShowing = 'Route: ridge loop from the north\nBring the stove\n';
  const notesOther = 'Route: lake path from the south\nBring the stove\n';
  const samples = new Map<string, Sample>([
    [
      'mock-conflict-entry',
      {
        row: {
          id: 'mock-conflict-entry',
          subject: { kind: 'entry', page_date: '2026-10-21', written_time: '09:12' },
          module: 'journal',
          raised_at: at(9, 40),
          showing: { device: 'Alex’s MacBook Pro', at: at(9, 40) },
          other: { device: 'Pixel 8 (simulated)', at: at(9, 31) },
        },
        base: 'Went for a run along the canal.\nCoffee with Sam after.\n',
        showing: entryShowing,
        other: entryOther,
        open: true,
        resolution: null,
      },
    ],
    [
      'mock-conflict-notes',
      {
        row: {
          id: 'mock-conflict-notes',
          subject: { kind: 'task', title: 'Plan weekend hike', field: 'notes' },
          module: 'todo',
          raised_at: at(8, 5),
          showing: { device: 'Pixel 8 (simulated)', at: at(8, 5) },
          other: { device: 'Alex’s MacBook Pro', at: at(7, 58) },
        },
        base: 'Route: to decide\nBring the stove\n',
        showing: notesShowing,
        other: notesOther,
        open: true,
        resolution: null,
      },
    ],
  ]);

  for (const id of samples.keys()) sampleIds.add(id);
  const shown = (id: string) => !sampleIds.has(id) || paired();
  const openSamples = () => [...samples.values()].filter((s) => s.open && shown(s.row.id));

  const view = (s: Sample): ConflictView => {
    const [showing, other] = diffSpans(showingNow(s), s.other);
    return {
      row: s.row,
      open: s.open,
      resolution: s.resolution,
      showing: showing ?? [],
      other: other ?? [],
      original: s.base,
      pieces: piecesOf(s),
    };
  };

  return {
    /** A Conflict a sync found (false when this clash is already listed). */
    raise(sample: Sample): boolean {
      if (samples.has(sample.row.id)) return false;
      samples.set(sample.row.id, sample);
      emit();
      return true;
    },
    /** Rows a sync added to "Resolved automatically", newest first. */
    addResolved(rows: ResolvedRow[]) {
      const known = new Set(resolved.map((r) => r.id));
      for (const r of rows) if (!known.has(r.id)) resolved.unshift(r);
      emit();
    },
    /** How many conflicts wait, for the attention the Home card and badge read. */
    openCount: () => openSamples().length,
    /** Whether this Conflict still waits for the person (the Activity line's Review link). */
    isOpen: (id: string) => openSamples().some((s) => s.row.id === id),
    handles: (cmd: string) => COMMANDS.includes(cmd),
    handle(cmd: string, a: Record<string, unknown>): unknown {
      const sample = samples.get(String(a.id));
      switch (cmd) {
        case 'sync_conflicts':
          return {
            needs_you: openSamples().map((s) => s.row),
            resolved: resolved
              .filter((r) => shown(r.id))
              .map((r) => ({ ...r, changes: [...r.changes] })),
          } satisfies ConflictList;
        case 'sync_resolution_use_other': {
          const row = resolved.find((r) => r.id === String(a.id));
          if (!row?.action || row.used) throw gone('That cannot be changed');
          row.used = row.action.replace(/^Use (.*) instead$/, 'Changed to $1');
          row.action = null;
          emit();
          return null;
        }
        case 'sync_resolutions_visited':
          for (const r of resolved) r.fresh = false;
          return null;
        case 'sync_conflict':
          return sample ? view(sample) : null;
        case 'sync_conflict_draft':
          if (!sample) throw gone();
          return draftOf(piecesOf(sample), (a.choices as ClashChoice[]) ?? []);
        case 'sync_conflict_resolve': {
          if (!sample?.open) throw gone();
          const choice = a.choice as ConflictChoice;
          if (choice.kind === 'keep_other') sample.showing = sample.other;
          if (choice.kind === 'merged') sample.showing = choice.text;
          if (choice.kind !== 'keep_this') sample.apply?.(sample.showing);
          sample.open = false;
          sample.resolution = choice.kind === 'merged' ? 'merged' : choice.kind;
          emit();
          return null;
        }
        default:
          return undefined;
      }
    },
  };
}

const gone = (message = 'That conflict is gone'): AppError => ({
  code: 'validation',
  message,
  details: null,
});
