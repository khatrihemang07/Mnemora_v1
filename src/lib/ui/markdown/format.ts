import { type Edit, type TextChange, lineBounds, selectedLineStarts } from './edit';

export type InlineMark = '**' | '*' | '~~' | '==' | '`';
export type BlockKind = 'h1' | 'h2' | 'h3' | 'bullet' | 'numbered' | 'task' | 'quote';

/** A line's block prefix: indent, then a heading, quote, checklist, bullet or number marker. */
export const BLOCK_PREFIX =
  /^([ \t]*)(#{1,6}[ \t]+|>[ \t]?|[-*+][ \t]+\[[ xX]\][ \t]+|[-*+][ \t]+|\d+[.)][ \t]+)/;

const HEADING = /^#{1,6}[ \t]+/;
const TASK = /^[-*+][ \t]+\[[ xX]\][ \t]+/;
const BULLET = /^[-*+][ \t]+/;
const NUMBER = /^\d+[.)][ \t]+/;

/** The kind of block a line's prefix makes it, or null for plain text. */
export function blockKind(line: string): BlockKind | null {
  const m = BLOCK_PREFIX.exec(line);
  const marker = m?.[2];
  if (marker === undefined) return null;
  if (HEADING.test(marker)) {
    const level = marker.trim().length;
    return level <= 3 ? (`h${level}` as BlockKind) : null;
  }
  if (marker.startsWith('>')) return 'quote';
  if (TASK.test(marker)) return 'task';
  if (BULLET.test(marker)) return 'bullet';
  if (NUMBER.test(marker)) return 'numbered';
  return null;
}

const PREFIXES: Record<BlockKind, (n: number) => string> = {
  h1: () => '# ',
  h2: () => '## ',
  h3: () => '### ',
  bullet: () => '- ',
  numbered: (n) => `${n}. `,
  task: () => '- [ ] ',
  quote: () => '> ',
};
const KEEPS_INDENT = new Set<BlockKind>(['bullet', 'numbered', 'task']);

function surround(doc: string, pos: number, ch: string, dir: -1 | 1): number {
  let n = 0;
  while (doc[dir === -1 ? pos - 1 - n : pos + n] === ch) n += 1;
  return n;
}

/** Whether `from..to` already sits inside `mark`. `*` and `**` share a character, so count runs. */
function wrappedBy(doc: string, from: number, to: number, mark: InlineMark): boolean {
  if (mark !== '*' && mark !== '**') {
    return doc.slice(from - mark.length, from) === mark && doc.slice(to, to + mark.length) === mark;
  }
  const run = Math.min(surround(doc, from, '*', -1), surround(doc, to, '*', 1));
  return mark === '*' ? run % 2 === 1 : run >= 2;
}

/** Wraps the selection in `mark`, or removes it when already wrapped. A caret gets an empty pair. */
export function toggleInline(doc: string, from: number, to: number, mark: InlineMark): Edit {
  const len = mark.length;
  if (from === to) {
    if (wrappedBy(doc, from, to, mark)) {
      return {
        changes: [
          { from: from - len, to: from, insert: '' },
          { from: to, to: to + len, insert: '' },
        ],
      };
    }
    return {
      changes: [{ from, to, insert: mark + mark }],
      selection: { anchor: from + len, head: from + len },
    };
  }

  // Emphasis cannot start or end with a space, so keep the marks tight around the words.
  const selected = doc.slice(from, to);
  const lead = selected.length - selected.trimStart().length;
  const trail = selected.length - selected.trimEnd().length;
  if (lead + trail >= selected.length) return { changes: [] };
  const a = from + lead;
  const b = to - trail;

  if (wrappedBy(doc, a, b, mark)) {
    return {
      changes: [
        { from: a - len, to: a, insert: '' },
        { from: b, to: b + len, insert: '' },
      ],
      selection: { anchor: a - len, head: b - len },
    };
  }
  return {
    changes: [
      { from: a, to: a, insert: mark },
      { from: b, to: b, insert: mark },
    ],
    selection: { anchor: a + len, head: b + len },
  };
}

/**
 * Turns every non-blank selected line into `kind`, or back into plain text when they all already
 * are. Numbered lines count up from 1. A lone blank line is included so a caret can start a block.
 */
export function toggleBlock(doc: string, from: number, to: number, kind: BlockKind): Edit {
  const starts = selectedLineStarts(doc, from, to);
  const lines = starts.map((start) => ({
    start,
    text: doc.slice(start, lineBounds(doc, start).end),
  }));
  let targets = lines.filter((l) => l.text.trim() !== '');
  if (targets.length === 0 && lines.length === 1) targets = lines;
  const remove = targets.length > 0 && targets.every((l) => blockKind(l.text) === kind);

  let n = 0;
  const changes = targets.map((l) => {
      const m = BLOCK_PREFIX.exec(l.text);
      const indent = /^[ \t]*/.exec(l.text)?.[0] ?? '';
      const keepIndent = KEEPS_INDENT.has(kind) && !remove;
      // Headings and quotes start at the margin; lists keep their nesting.
      const oldLength = m ? m[0].length : keepIndent ? 0 : indent.length;
      n += 1;
      return {
        from: l.start,
        to: l.start + oldLength,
        insert: remove ? '' : (keepIndent && m ? m[1] : '') + PREFIXES[kind](n),
      };
  });
  return from === to ? { changes, selection: caretAfter(changes, from) } : { changes };
}

/**
 * Where a caret ends up after `changes`: a change that ends at or before it moves it, so a marker
 * inserted AT the caret (an empty line, or the start of a line) leaves the caret after it; a
 * replaced prefix that held the caret puts it after the new text.
 */
function caretAfter(changes: TextChange[], pos: number): { anchor: number; head: number } {
  let caret = pos;
  for (const c of changes) {
    if (c.to <= pos) caret += c.insert.length - (c.to - c.from);
    else if (c.from < pos) caret = c.from + c.insert.length;
  }
  return { anchor: caret, head: caret };
}

const URL_ONLY = /^https?:\/\/\S+$/i;

/**
 * Turns the selection into a link and selects the part to fill in. Needs a selection: with none it
 * returns null so ⌘K still opens the command palette.
 */
export function makeLink(doc: string, from: number, to: number): Edit | null {
  const selected = doc.slice(from, to).trim();
  if (selected === '') return null;
  if (URL_ONLY.test(selected)) {
    return {
      changes: [{ from, to, insert: `[](${selected})` }],
      selection: { anchor: from + 1, head: from + 1 },
    };
  }
  const urlStart = from + selected.length + 3;
  return {
    changes: [{ from, to, insert: `[${selected}](url)` }],
    selection: { anchor: urlStart, head: urlStart + 3 },
  };
}

/** A link with nothing selected (the phone bar's link button): `[text](url)` with "text" selected. */
export function linkTemplate(pos: number): Edit {
  return {
    changes: [{ from: pos, to: pos, insert: '[text](url)' }],
    selection: { anchor: pos + 1, head: pos + 5 },
  };
}
