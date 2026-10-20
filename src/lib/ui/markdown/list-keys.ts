import { type Edit, lineBounds, selectedLineStarts } from './edit';
import { BLOCK_PREFIX } from './format';

/** A list item line: indent, bullet or number, spacing, optional checkbox. */
const ITEM = /^([ \t]*)([-*+]|(\d+)([.)]))([ \t]+)(\[[ xX]\][ \t]+)?/;
const BULLET_INDENT = 2;

function lineAt(doc: string, pos: number) {
  const { start, end } = lineBounds(doc, pos);
  return { start, end, text: doc.slice(start, end) };
}

/** How far a nested item is indented: the width of its own marker ("- " is 2, "10. " is 4). */
function indentUnit(m: RegExpExecArray): number {
  return m[3] === undefined ? BULLET_INDENT : (m[3] + (m[4] ?? '.')).length + 1;
}

/**
 * Enter in a list item. A filled item starts the next one (same bullet, next number, a fresh
 * unticked checkbox); an empty item ends the list (one level out first when nested). Returns null
 * outside list items, or with the caret inside the marker, for the default Enter.
 */
export function listEnter(doc: string, pos: number): Edit | null {
  const line = lineAt(doc, pos);
  const m = ITEM.exec(line.text);
  if (!m || pos < line.start + m[0].length) return null;

  const indent = m[1] ?? '';
  if (line.text.slice(m[0].length).trim() === '') {
    if (indent !== '') {
      const cut = Math.min(indent.length, indentUnit(m));
      return { changes: [{ from: line.start, to: line.start + cut, insert: '' }] };
    }
    return {
      changes: [{ from: line.start, to: line.end, insert: '' }],
      selection: { anchor: line.start, head: line.start },
    };
  }

  const bullet = m[3] === undefined ? (m[2] ?? '-') : `${Number(m[3]) + 1}${m[4] ?? '.'}`;
  const insert = `\n${indent}${bullet}${m[5] ?? ' '}${m[6] ? '[ ] ' : ''}`;
  return {
    changes: [{ from: pos, to: pos, insert }],
    selection: { anchor: pos + insert.length, head: pos + insert.length },
  };
}

/**
 * Tab / Shift+Tab: nests or lifts every selected list item. Returns null when no selected line is
 * a list item, so Tab keeps its normal job (moving focus on).
 */
export function listIndent(doc: string, from: number, to: number, outdent: boolean): Edit | null {
  const changes: Edit['changes'] = [];
  let any = false;
  for (const start of selectedLineStarts(doc, from, to)) {
    const text = doc.slice(start, lineBounds(doc, start).end);
    const m = ITEM.exec(text);
    if (!m) continue;
    any = true;
    const unit = indentUnit(m);
    if (!outdent) {
      changes.push({ from: start, to: start, insert: ' '.repeat(unit) });
      continue;
    }
    const lead = /^[ \t]*/.exec(text)?.[0] ?? '';
    if (lead === '') continue;
    const cut = lead.startsWith('\t') ? 1 : Math.min(unit, lead.length);
    changes.push({ from: start, to: start + cut, insert: '' });
  }
  return any ? { changes } : null;
}

/**
 * Backspace with the caret right after a block marker (list, checklist, quote or heading) removes
 * the marker and its indent, leaving the text. Null elsewhere, for the normal Backspace.
 */
export function markerBackspace(doc: string, pos: number): Edit | null {
  const line = lineAt(doc, pos);
  const m = BLOCK_PREFIX.exec(line.text);
  if (!m || pos !== line.start + m[0].length) return null;
  return {
    changes: [{ from: line.start, to: pos, insert: '' }],
    selection: { anchor: line.start, head: line.start },
  };
}
