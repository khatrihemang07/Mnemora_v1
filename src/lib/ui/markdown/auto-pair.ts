import type { Edit } from './edit';

/** Opener to closer. Quotes and underscores are never auto-closed. */
const CLOSERS: Record<string, string> = { '*': '*', '`': '`', '(': ')', '[': ']' };
const STEP_OVER = new Set([')', ']']);
/** A pair opens only before the end of the text, a space or closing punctuation. */
const CLOSES_BEFORE = /^$|[\s)\]}.,;:!?]/;
const WORD = /[\p{L}\p{N}_]/u;

const caretAt = (pos: number): Edit => ({
  changes: [],
  selection: { anchor: pos, head: pos },
});

/**
 * Handles typing `typed` at the selection `from..to`. Returns an edit when the auto-pair rules
 * apply (wrap a selection, step over a closer, open a pair), or null to type the character as is.
 * Pairs are `**`, a backtick, `(` and `[`; a single `*` is only typed, so `* item` lists still work.
 */
export function autoPair(doc: string, from: number, to: number, typed: string): Edit | null {
  const close = CLOSERS[typed];
  if (from !== to) {
    if (close === undefined) return null;
    return {
      changes: [
        { from, to: from, insert: typed },
        { from: to, to, insert: close },
      ],
      selection: { anchor: from + 1, head: to + 1 },
    };
  }

  const prev = doc[from - 1] ?? '';
  const next = doc[from] ?? '';

  if (STEP_OVER.has(typed)) return next === typed ? caretAt(from + 1) : null;

  if (typed === '*') {
    if (next === '*' && prev !== '' && !/\s/.test(prev)) return caretAt(from + 1);
    // The second `*` of `**` closes it at once: `*|` becomes `**|**`.
    if (prev === '*' && doc[from - 2] !== '*' && CLOSES_BEFORE.test(next)) {
      return {
        changes: [{ from, to: from, insert: '***' }],
        selection: { anchor: from + 1, head: from + 1 },
      };
    }
    return null;
  }

  if (typed === '`') {
    if (next === '`' && prev !== '' && !/\s/.test(prev)) return caretAt(from + 1);
    if (prev === '`' || WORD.test(prev) || !CLOSES_BEFORE.test(next)) return null;
  } else if (close === undefined || !CLOSES_BEFORE.test(next)) {
    return null;
  }
  return {
    changes: [{ from, to: from, insert: typed + (close ?? '') }],
    selection: { anchor: from + 1, head: from + 1 },
  };
}

/** Backspace between an empty pair (`(|)`, `[|]`, `` `|` ``, `*|*`, `**|**`) deletes both halves. */
export function pairBackspace(doc: string, pos: number): Edit | null {
  if (doc.slice(pos - 2, pos) === '**' && doc.slice(pos, pos + 2) === '**') {
    return { changes: [{ from: pos - 2, to: pos + 2, insert: '' }] };
  }
  const prev = doc[pos - 1];
  const next = doc[pos];
  if (prev === undefined || next === undefined || CLOSERS[prev] !== next) return null;
  if (prev === '*' && (doc[pos - 2] === '*' || doc[pos + 1] === '*')) return null;
  return { changes: [{ from: pos - 1, to: pos + 1, insert: '' }] };
}
