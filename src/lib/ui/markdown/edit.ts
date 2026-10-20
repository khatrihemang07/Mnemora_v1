/** One replacement, in the coordinates of the document before the edit. */
export interface TextChange {
  from: number;
  to: number;
  insert: string;
}

/**
 * A pure text edit: what the editor helpers return, so they can be tested without CodeMirror. When
 * `selection` is missing the editor maps the existing selection through `changes`.
 */
export interface Edit {
  changes: TextChange[];
  /** New selection, in coordinates after the changes. */
  selection?: { anchor: number; head: number };
}

/** Start and end (before the newline) of the line holding `pos`. */
export function lineBounds(doc: string, pos: number): { start: number; end: number } {
  const start = doc.lastIndexOf('\n', pos - 1) + 1;
  const nl = doc.indexOf('\n', pos);
  return { start, end: nl === -1 ? doc.length : nl };
}

/** Starts of every line touched by the selection; a selection ending at a line start skips it. */
export function selectedLineStarts(doc: string, from: number, to: number): number[] {
  const starts: number[] = [];
  let pos = lineBounds(doc, from).start;
  const last = to > from ? lineBounds(doc, to - 1).start : pos;
  while (pos <= last) {
    starts.push(pos);
    const end = lineBounds(doc, pos).end;
    if (end >= doc.length) break;
    pos = end + 1;
  }
  return starts;
}

/** Applies an edit to `doc`, for tests. The selection defaults to the caret at the end. */
export function applyEdit(doc: string, edit: Edit): { doc: string; anchor: number; head: number } {
  const sorted = [...edit.changes].sort((a, b) => b.from - a.from);
  let out = doc;
  for (const c of sorted) out = out.slice(0, c.from) + c.insert + out.slice(c.to);
  const sel = edit.selection ?? { anchor: out.length, head: out.length };
  return { doc: out, anchor: sel.anchor, head: sel.head };
}
