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
} from './bindings';

interface Sample {
  row: ConflictRow;
  base: string;
  showing: string;
  other: string;
  pieces: ClashPiece[];
  open: boolean;
  resolution: string | null;
}

const COMMANDS = [
  'sync_conflicts',
  'sync_conflict',
  'sync_conflict_draft',
  'sync_conflict_resolve',
];

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

export function createConflictsMock(emit: () => void = () => {}) {
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
        pieces: [
          { kind: 'agreed', text: 'Went for a run along the canal, ' },
          {
            kind: 'clash',
            showing: 'felt great after the rain.',
            other: 'legs heavy but good.',
            original: 'canal.',
          },
          { kind: 'agreed', text: '\nCoffee with Sam after.\n' },
        ],
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
        pieces: [
          { kind: 'agreed', text: 'Route: ' },
          {
            kind: 'clash',
            showing: 'ridge loop from the north',
            other: 'lake path from the south',
            original: 'to decide',
          },
          { kind: 'agreed', text: '\nBring the stove\n' },
        ],
        open: true,
        resolution: null,
      },
    ],
  ]);

  const view = (s: Sample): ConflictView => {
    const [showing, other] = diffSpans(s.showing, s.other);
    return {
      row: s.row,
      open: s.open,
      resolution: s.resolution,
      showing: showing ?? [],
      other: other ?? [],
      original: s.base,
      pieces: s.pieces,
    };
  };

  return {
    handles: (cmd: string) => COMMANDS.includes(cmd),
    handle(cmd: string, a: Record<string, unknown>): unknown {
      const sample = samples.get(String(a.id));
      switch (cmd) {
        case 'sync_conflicts':
          return {
            needs_you: [...samples.values()].filter((s) => s.open).map((s) => s.row),
          } satisfies ConflictList;
        case 'sync_conflict':
          return sample ? view(sample) : null;
        case 'sync_conflict_draft':
          if (!sample) throw gone();
          return draftOf(sample.pieces, (a.choices as ClashChoice[]) ?? []);
        case 'sync_conflict_resolve': {
          if (!sample?.open) throw gone();
          const choice = a.choice as ConflictChoice;
          if (choice.kind === 'keep_other') sample.showing = sample.other;
          if (choice.kind === 'merged') sample.showing = choice.text;
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

const gone = (): AppError => ({
  code: 'validation',
  message: 'That conflict is gone',
  details: null,
});
