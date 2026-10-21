/** The geometry and list arithmetic of dragging Tasks (and Lists) into place. Pure, no DOM. */

/** A row's place on screen. */
export interface RowBox {
  id: string;
  top: number;
  bottom: number;
}

/** The ids on either side of a place in a list: what `todo_task_reorder` takes. */
export interface Neighbours {
  /** Directly above the place, or none at the top. */
  before: string | null;
  /** Directly below it, or none at the bottom. */
  after: string | null;
}

/**
 * Where among `rows` (top to bottom, the dragged one left out) the pointer at `y` would drop: an
 * index from 0 (above the first) to `rows.length` (below the last). A row's upper half is above it.
 */
export function insertIndex(rows: readonly RowBox[], y: number): number {
  const at = rows.findIndex((r) => y < (r.top + r.bottom) / 2);
  return at === -1 ? rows.length : at;
}

/** The ids around an index of `ids`. */
export function neighboursAt(ids: readonly string[], index: number): Neighbours {
  return { before: ids[index - 1] ?? null, after: ids[index] ?? null };
}

/**
 * The neighbours after moving `id` one place up (-1) or down (1) among `ids`, or none when it
 * cannot move that way (it is the first or last, or not there).
 */
export function stepNeighbours(ids: readonly string[], id: string, dir: -1 | 1): Neighbours | null {
  const from = ids.indexOf(id);
  if (from === -1) return null;
  const to = from + dir;
  if (to < 0 || to >= ids.length) return null;
  const others = ids.filter((i) => i !== id);
  return neighboursAt(others, to);
}

/** Whether placing `id` between these neighbours leaves it where it already is in `ids`. */
export function isSamePlace(ids: readonly string[], id: string, at: Neighbours): boolean {
  const from = ids.indexOf(id);
  if (from === -1) return false;
  return (ids[from - 1] ?? null) === at.before && (ids[from + 1] ?? null) === at.after;
}

/** Which way, and how fast, to scroll while dragging near the edge of a scroller (px per frame). */
export function autoScrollSpeed(
  y: number,
  top: number,
  bottom: number,
  edge = 56,
  max = 16,
): number {
  if (y < top + edge) return -Math.round(max * Math.min(1, (top + edge - y) / edge));
  if (y > bottom - edge) return Math.round(max * Math.min(1, (y - (bottom - edge)) / edge));
  return 0;
}
