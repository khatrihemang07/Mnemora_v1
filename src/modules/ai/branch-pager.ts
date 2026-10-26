/** The pager's words for a question among its siblings, such as "2 of 3" (`index` counts from 1). */
export function pagerLabel(index: number, count: number): string {
  return `${index} of ${count}`;
}

/**
 * The position the pager moves to from `index` among `count` siblings, or null when there is no
 * question that way (at either end, or with no siblings).
 */
export function pagerStep(
  index: number,
  count: number,
  direction: 'previous' | 'next',
): number | null {
  const target = direction === 'previous' ? index - 1 : index + 1;
  return target >= 1 && target <= count ? target : null;
}
