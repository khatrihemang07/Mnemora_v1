/** The pager's words for a question among its siblings, as the prototype has them: "Question 2 of 3" (`index` counts from 1). */
export function pagerLabel(index: number, count: number): string {
  return `Question ${index} of ${count}`;
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

/**
 * Where focus goes after the pager moved: the arrow pressed while it is still on, otherwise the other
 * arrow, or nowhere when both are off.
 */
export function pagerFocusAfterShow(
  pressed: 'previous' | 'next',
  enabled: { previous: boolean; next: boolean },
): 'previous' | 'next' | null {
  const other = pressed === 'next' ? 'previous' : 'next';
  if (enabled[pressed]) return pressed;
  if (enabled[other]) return other;
  return null;
}
