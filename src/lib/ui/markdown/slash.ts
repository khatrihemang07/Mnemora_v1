import { type Edit, lineBounds } from './edit';

export interface SlashItem {
  id: string;
  label: string;
  /** Extra words that also match what the writer types after the "/". */
  keywords: string;
  /** Block items start their own line; inline items insert text where the "/" was. */
  kind: 'block' | 'inline';
}

export const SLASH_ITEMS: readonly SlashItem[] = [
  { id: 'h1', label: 'Heading 1', keywords: 'h1 title', kind: 'block' },
  { id: 'h2', label: 'Heading 2', keywords: 'h2 subtitle', kind: 'block' },
  { id: 'h3', label: 'Heading 3', keywords: 'h3', kind: 'block' },
  { id: 'bullet', label: 'Bulleted list', keywords: 'bullet list ul', kind: 'block' },
  { id: 'numbered', label: 'Numbered list', keywords: 'numbered ordered list ol', kind: 'block' },
  { id: 'task', label: 'Checklist', keywords: 'checklist todo task checkbox', kind: 'block' },
  { id: 'quote', label: 'Quote', keywords: 'quote blockquote', kind: 'block' },
  { id: 'divider', label: 'Divider', keywords: 'divider rule line hr', kind: 'block' },
  { id: 'code', label: 'Code block', keywords: 'code fence pre', kind: 'block' },
  { id: 'date', label: "Today's date", keywords: 'date today', kind: 'inline' },
  { id: 'time', label: 'Current time', keywords: 'time now clock', kind: 'inline' },
];

/** What the slash menu needs from the app: the date and time as the General Settings show them. */
export interface SlashValues {
  date: string;
  time: string;
}

/** The menu may open for a "/" at the start of a line or right after whitespace. */
export function canOpenSlash(doc: string, slashPos: number): boolean {
  if (doc[slashPos] !== '/') return false;
  return slashPos === 0 || /\s/.test(doc[slashPos - 1] ?? '');
}

/**
 * What the writer has typed after the "/" while the menu is open, or null when it should be closed:
 * the "/" is gone, the caret left it, or a space was typed (so "and/or" and "a / b" stay text).
 */
export function slashQuery(doc: string, slashPos: number, caret: number): string | null {
  if (doc[slashPos] !== '/' || caret <= slashPos) return null;
  const typed = doc.slice(slashPos + 1, caret);
  return /\s/.test(typed) ? null : typed;
}

/** Items matching the typed text (a prefix of the label or of any keyword), in menu order. */
export function filterSlash(query: string): SlashItem[] {
  const q = query.trim().toLowerCase();
  if (q === '') return [...SLASH_ITEMS];
  return SLASH_ITEMS.filter((item) =>
    `${item.label} ${item.keywords}`
      .toLowerCase()
      .split(/\s+/)
      .some((w) => w.startsWith(q)),
  );
}

const BLOCK_INSERT: Record<string, { text: string; caret: number }> = {
  h1: { text: '# ', caret: 2 },
  h2: { text: '## ', caret: 3 },
  h3: { text: '### ', caret: 4 },
  bullet: { text: '- ', caret: 2 },
  numbered: { text: '1. ', caret: 3 },
  task: { text: '- [ ] ', caret: 6 },
  quote: { text: '> ', caret: 2 },
  divider: { text: '---\n', caret: 4 },
  code: { text: '```\n\n```', caret: 4 },
};

/** A line that is only a list or quote marker: the picked block replaces it. */
const MARKER_ONLY = /^[ \t]*(?:>[ \t]?|[-*+][ \t]*(?:\[[ xX]\][ \t]*)?|\d+[.)][ \t]*)$/;

/**
 * The edit for picking `item` while the menu is open from `slashPos` to `caret`. Inline items
 * replace the "/…" with text. A block item replaces it too; picked after other text on the line it
 * starts on a new line, so the sentence never changes type.
 */
export function applySlash(
  doc: string,
  slashPos: number,
  caret: number,
  item: SlashItem,
  values: SlashValues,
): Edit {
  if (item.kind === 'inline') {
    const text = item.id === 'date' ? values.date : values.time;
    return {
      changes: [{ from: slashPos, to: caret, insert: text }],
      selection: { anchor: slashPos + text.length, head: slashPos + text.length },
    };
  }

  const block = BLOCK_INSERT[item.id];
  if (!block) return { changes: [] };
  const line = lineBounds(doc, slashPos);
  const before = doc.slice(line.start, slashPos);
  const afterEnd = Math.min(line.end, doc.length);
  // Whatever follows the menu on the line moves with the new block, minus the gap before it.
  const trailing = doc.slice(caret, afterEnd);
  const trimmedTrailing = trailing.replace(/^[ \t]+/, '');
  const dropped = trailing.length - trimmedTrailing.length;

  if (before.trim() === '' || MARKER_ONLY.test(before)) {
    // Start of a (possibly marker-only) line: the block takes the line.
    const from = line.start;
    const insert = block.text;
    return {
      changes: [{ from, to: caret + dropped, insert }],
      selection: { anchor: from + block.caret, head: from + block.caret },
    };
  }

  const keep = before.replace(/[ \t]+$/, '');
  const insert = `\n${block.text}`;
  const from = line.start + keep.length;
  return {
    changes: [{ from, to: caret + dropped, insert }],
    selection: { anchor: from + 1 + block.caret, head: from + 1 + block.caret },
  };
}
