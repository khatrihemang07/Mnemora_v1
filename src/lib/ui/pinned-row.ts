/** Which row a virtual list pins at its top, and how far the next pinned row pushes it up. */

export interface RowStart {
  index: number;
  start: number;
}

/**
 * The last sticky row starting at or above `scrollTop` is pinned. When the next sticky row comes
 * within `height` px of the top, the pinned copy slides up so the two never overlap.
 */
export function pinnedRow(
  rows: RowStart[],
  scrollTop: number,
  isSticky: (index: number) => boolean,
  height: number,
): { index: number; offset: number } | null {
  const sticky = rows.filter((r) => isSticky(r.index)).sort((a, b) => a.start - b.start);
  let at = -1;
  for (const [i, r] of sticky.entries()) if (r.start <= scrollTop) at = i;
  const current = sticky[at];
  if (!current) return null;
  const next = sticky[at + 1];
  const gap = next ? next.start - scrollTop : Number.POSITIVE_INFINITY;
  return { index: current.index, offset: Math.min(0, gap - height) };
}
