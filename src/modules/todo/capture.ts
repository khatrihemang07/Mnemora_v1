/**
 * The capture box's rules, as pure helpers: which words are kept as text or swapped ("use
 * instead"), what ⌫ right after a recognised word does, the `#List` suggestions and the keys that
 * drive them, which stretches of an old title Quick edit must not read again, how a multi-line
 * paste becomes one Task, and how the typed words and the property chips settle the fields.
 */
import type { Parsed, ParseSpan, RepeatRule, TextRange } from '../../lib/ipc/bindings';

/** Longest title; Rust enforces it too. */
export const MAX_TITLE = 500;

// --- Words kept as text, and "use instead" ---------------------------------------------------------

/** What the user has decided about the words the parser found. */
export interface CaptureWords {
  /** Keys of words kept as text. */
  ignored: string[];
  /** Per kind, the key of the word chosen over the last one. */
  prefer: Record<string, string>;
}

export const NO_WORDS: CaptureWords = { ignored: [], prefer: {} };

const without = (prefer: Record<string, string>, key: string): Record<string, string> =>
  Object.fromEntries(Object.entries(prefer).filter(([, v]) => v !== key));

/** Keeps a word as text (× on its chip, or ⌫ right after it). */
export function ignoreWord(words: CaptureWords, key: string): CaptureWords {
  if (words.ignored.includes(key)) return words;
  return { ignored: [...words.ignored, key], prefer: without(words.prefer, key) };
}

/** Reads a word again (the "kept as text ↺" chip). */
export function recogniseAgain(words: CaptureWords, key: string): CaptureWords {
  return { ...words, ignored: words.ignored.filter((k) => k !== key) };
}

/** Chooses a word over the one that would win ("use instead"). */
export function preferWord(
  words: CaptureWords,
  span: Pick<ParseSpan, 'kind' | 'key'>,
): CaptureWords {
  return { ...words, prefer: { ...words.prefer, [span.kind]: span.key } };
}

/**
 * The user set these kinds by hand (a property chip): the words of those kinds in the text become
 * text, so the choice is not overruled by them, and any "use instead" for them is forgotten.
 */
export function ignoreKinds(
  words: CaptureWords,
  parsed: Pick<Parsed, 'spans' | 'alternatives'> | null,
  kinds: string[],
): CaptureWords {
  const found = [...(parsed?.spans ?? []), ...(parsed?.alternatives ?? [])].filter((s) =>
    kinds.includes(s.kind),
  );
  const prefer = Object.fromEntries(
    Object.entries(words.prefer).filter(([k]) => !kinds.includes(k)),
  );
  return { ignored: [...new Set([...words.ignored, ...found.map((s) => s.key)])], prefer };
}

/** Words that no longer appear in the text cannot be told apart from new ones: forget them. */
export function pruneWords(words: CaptureWords, known: ParseSpan[]): CaptureWords {
  const keys = new Set(known.map((s) => s.key));
  const ignored = words.ignored.filter((k) => keys.has(k));
  const prefer = Object.fromEntries(Object.entries(words.prefer).filter(([, k]) => keys.has(k)));
  return ignored.length === words.ignored.length &&
    Object.keys(prefer).length === Object.keys(words.prefer).length
    ? words
    : { ignored, prefer };
}

// --- ⌫ right after a recognised word ---------------------------------------------------------------

export interface BackspaceInput {
  text: string;
  /** The selection; ⌫ only un-recognises with a caret, never with a selection. */
  selectionStart: number;
  selectionEnd: number;
  /** The words that won, for this very text. */
  spans: Pick<ParseSpan, 'key' | 'end'>[];
  /** The last ⌫ just un-recognised a word: this one deletes. */
  justUnrecognised: boolean;
}

/**
 * The word ⌫ should keep as text instead of deleting a character: the caret is right after it, or
 * after the one space that follows it. The first ⌫ un-recognises; the second deletes as usual.
 */
export function backspaceTarget(i: BackspaceInput): string | null {
  if (i.justUnrecognised || i.selectionStart !== i.selectionEnd) return null;
  const caret = i.selectionStart;
  const hit = i.spans.find(
    (s) => s.end === caret || (s.end === caret - 1 && i.text[caret - 1] === ' '),
  );
  return hit?.key ?? null;
}

// --- #List suggestions -----------------------------------------------------------------------------

const TAG_CHAR = /[\p{L}\p{N}_-]/u;
const FOLD_EXTRA: Record<string, string> = { đ: 'd', ø: 'o', ł: 'l', ı: 'i' };

/** Lowercase with accents removed, as Rust folds a List name. */
export const foldTag = (s: string): string =>
  [...s.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase()]
    .map((c) => FOLD_EXTRA[c] ?? c)
    .join('');

/** The part of a List name a `#tag` can spell. */
export const tagOf = (name: string): string => [...name].filter((c) => TAG_CHAR.test(c)).join('');

const tagKey = (name: string): string => foldTag(tagOf(name));

/** The `#word` the caret ends: where its `#` is and what has been typed after it. */
export interface HashQuery {
  start: number;
  query: string;
}

/** The `#tag` being typed at the caret, or none. The caret must be at the end of the word. */
export function hashQuery(text: string, caret: number): HashQuery | null {
  const before = text.slice(0, caret);
  const m = /(^|\s)#([\p{L}\p{N}_-]*)$/u.exec(before);
  if (!m) return null;
  const next = text[caret];
  if (next !== undefined && !/\s/.test(next)) return null;
  const query = m[2] ?? '';
  return { start: caret - query.length - 1, query };
}

export interface SuggestList {
  id: string;
  name: string;
  emoji: string | null;
}

/** A row in the suggestions: a List, or the offer to make a new one. */
export type Suggestion =
  | { kind: 'list'; id: string; name: string; emoji: string | null }
  | { kind: 'new'; name: string };

/**
 * The Lists whose names start with what was typed (exact names first), or, when none does, the
 * offer of a new List. A bare `#`, and a tag of digits only (an issue number), suggest nothing new.
 */
export function listSuggestions(query: string, lists: SuggestList[]): Suggestion[] {
  const typed = foldTag(query);
  const matching = lists.filter((l) => tagKey(l.name).startsWith(typed));
  if (matching.length > 0) {
    const exact = matching.filter((l) => tagKey(l.name) === typed);
    return [...exact, ...matching.filter((l) => !exact.includes(l))].map((l) => ({
      kind: 'list' as const,
      id: l.id,
      name: l.name,
      emoji: l.emoji,
    }));
  }
  if (!query || /^\d+$/.test(query)) return [];
  return [{ kind: 'new', name: query }];
}

export type SuggestionKey =
  | { type: 'move'; index: number }
  | { type: 'accept' }
  | { type: 'dismiss' };

/**
 * What a key does while suggestions show: ↑ ↓ move (round the ends), Enter and Tab accept the
 * highlighted row (so Enter never adds the Task), Esc dismisses them. Other keys are the text's.
 */
export function suggestionKey(
  key: string,
  shift: boolean,
  state: { count: number; index: number },
): SuggestionKey | null {
  if (state.count === 0) return null;
  switch (key) {
    case 'ArrowDown':
      return { type: 'move', index: (state.index + 1) % state.count };
    case 'ArrowUp':
      return { type: 'move', index: (state.index - 1 + state.count) % state.count };
    case 'Enter':
    case 'Tab':
      return shift ? null : { type: 'accept' };
    case 'Escape':
      return { type: 'dismiss' };
    default:
      return null;
  }
}

/** The text with the `#tag` completed to the suggestion and a space after it, and the new caret. */
export function applySuggestion(
  text: string,
  hash: HashQuery,
  suggestion: Suggestion,
): { text: string; caret: number } {
  const tag = tagOf(suggestion.name) || hash.query;
  const before = text.slice(0, hash.start);
  const after = text.slice(hash.start + 1 + hash.query.length);
  const spaced = after.startsWith(' ') ? after : ` ${after}`;
  const done = `${before}#${tag}`;
  return { text: done + spaced, caret: done.length + 1 };
}

// --- Highlighting ----------------------------------------------------------------------------------

/** A stretch of the typed text, marked when the parser recognised it. */
export interface Segment {
  text: string;
  mark: boolean;
}

/** The text cut into plain and recognised stretches, for the highlight behind the field. */
export function markSegments(text: string, spans: Pick<ParseSpan, 'start' | 'end'>[]): Segment[] {
  const out: Segment[] = [];
  let at = 0;
  for (const s of [...spans].sort((a, b) => a.start - b.start)) {
    if (s.start < at || s.end > text.length) continue;
    if (s.start > at) out.push({ text: text.slice(at, s.start), mark: false });
    out.push({ text: text.slice(s.start, s.end), mark: true });
    at = s.end;
  }
  if (at < text.length) out.push({ text: text.slice(at), mark: false });
  return out;
}

// --- Quick edit: words already in the old title ----------------------------------------------------

const isSpace = (c: string | undefined): boolean => c !== undefined && /\s/.test(c);

/**
 * The stretches of the edited text that are the old title's words, untouched: the words before the
 * first change and after the last one, whole words only. Quick edit passes them to the parser so
 * that a week-old "tomorrow" is never read again; only the new text is.
 */
export function unchangedRanges(oldText: string, newText: string): TextRange[] {
  const max = Math.min(oldText.length, newText.length);
  let p = 0;
  while (p < max && oldText[p] === newText[p]) p++;
  // The prefix must end where a word does, in both texts; the suffix must start where one does.
  const wordEnd = (t: string, i: number) => i === t.length || isSpace(t[i]);
  while (p > 0 && !(wordEnd(oldText, p) && wordEnd(newText, p))) p--;
  let s = 0;
  while (s < max - p && oldText[oldText.length - 1 - s] === newText[newText.length - 1 - s]) s++;
  const wordStart = (t: string, i: number) => i === 0 || isSpace(t[i - 1]);
  while (
    s > 0 &&
    !(wordStart(oldText, oldText.length - s) && wordStart(newText, newText.length - s))
  ) {
    s--;
  }
  const out: TextRange[] = [];
  if (p > 0) out.push({ start: 0, end: p });
  if (s > 0) out.push({ start: newText.length - s, end: newText.length });
  return out;
}

// --- Pasting ---------------------------------------------------------------------------------------

/**
 * A multi-line paste is one Task: the first line is the title and the rest are its notes. Capture only:
 * Quick edit joins the lines into the title instead (`pasteIntoTitle`). A paste
 * with no line break is left to the field. A title over the limit spills into the notes.
 */
export function pasteParts(pasted: string): { title: string; notes: string } | null {
  const lines = pasted.replace(/\r\n?/g, '\n').replace(/\n+$/, '').split('\n');
  if (lines.length < 2) return null;
  const first = lines.findIndex((l) => l.trim() !== '');
  if (first < 0) return null;
  const head = (lines[first] as string).trim();
  const title = [...head].slice(0, MAX_TITLE).join('');
  const spill = [...head].slice(MAX_TITLE).join('');
  const rest = lines.slice(first + 1).join('\n');
  return { title, notes: [spill, rest].filter(Boolean).join('\n').trim() };
}

/**
 * A multi-line paste into Quick edit stays in the title: the lines are joined with single spaces
 * and replace the selection, and the title stays within the limit. (Capture is different on
 * purpose: there the first line is the title and the rest are notes, see `pasteParts`.) A paste
 * with no line break is left to the field; blank lines give null too.
 */
export function pasteIntoTitle(
  text: string,
  start: number,
  end: number,
  pasted: string,
): { text: string; caret: number } | null {
  const lines = pasted
    .replace(/\r\n?/g, '\n')
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean);
  if (lines.length < 2) return null;
  const room = Math.max(0, MAX_TITLE - [...text.slice(0, start)].length - [...text.slice(end)].length);
  const insert = [...lines.join(' ')].slice(0, room).join('');
  return { text: `${text.slice(0, start)}${insert}${text.slice(end)}`, caret: start + insert.length };
}

// --- What the Task gets ----------------------------------------------------------------------------

/** What a Task has before the text says anything: its place, or its own fields in Quick edit. */
export interface CaptureBase {
  listId: string | null;
  date: string | null;
  time: string | null;
  repeat: RepeatRule | null;
  priority: number;
}

/** What the property chips set by hand; undefined means not touched, null means cleared. */
export interface CaptureHand {
  listId?: string | null;
  date?: string | null;
  time?: string | null;
  repeat?: RepeatRule | null;
  priority?: number;
}

export interface CaptureFields extends CaptureBase {
  /** A List from `#Name` that does not exist yet: made when the Task is added. */
  newList: string | null;
  /** The kinds of field the typed words set. */
  fromText: string[];
}

/**
 * The fields the Task will get: the words win over a chip, a chip over where the box opened. A
 * time with no date keeps the day the box opened on (an Upcoming day), and only a box that opened
 * with no day takes the parser's today-or-tomorrow.
 */
export function resolveFields(
  parsed: Parsed | null,
  hand: CaptureHand,
  base: CaptureBase,
): CaptureFields {
  const spans = parsed?.spans ?? [];
  const has = (kind: string) => spans.some((s) => s.kind === kind);
  const inherited = hand.date !== undefined ? hand.date : base.date;
  const date = has('date') ? (parsed?.date ?? null) : (inherited ?? parsed?.date ?? null);
  return {
    date,
    time: parsed?.time ?? (hand.time !== undefined ? hand.time : base.time),
    repeat: parsed?.repeat ?? (hand.repeat !== undefined ? hand.repeat : base.repeat),
    priority: parsed?.priority ?? hand.priority ?? base.priority,
    listId:
      parsed?.list_id ??
      (parsed?.new_list ? null : hand.listId !== undefined ? hand.listId : base.listId),
    newList: parsed?.new_list ?? null,
    fromText: [...new Set(spans.map((s) => s.kind))],
  };
}
