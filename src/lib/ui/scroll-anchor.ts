/** The row being read, remembered so rows added above it can be compensated for. */
export interface ScrollAnchor {
  key: string | number | bigint;
  index: number;
  delta: number;
}

/**
 * Where to scroll so the anchored row stays put after the items changed: null when the row is gone
 * or has not moved, else its new start plus the distance the view was into it.
 */
export function anchoredScrollTop(
  anchor: ScrollAnchor | null,
  keys: (string | number | bigint)[],
  startOf: (index: number) => number | undefined,
): { top: number; index: number } | null {
  if (!anchor) return null;
  const index = keys.indexOf(anchor.key);
  if (index < 0 || index === anchor.index) return null;
  const start = startOf(index);
  return start === undefined ? null : { top: start + anchor.delta, index };
}
