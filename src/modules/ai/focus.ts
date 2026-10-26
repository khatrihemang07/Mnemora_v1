/** The part of an element that focus logic needs, so it can be tested without a DOM. */
export interface Focusable {
  readonly isConnected: boolean;
  focus(): void;
}

/**
 * Puts focus back where an overlay was opened from: the opener when it is still on screen, else
 * the fallback (the question box). A closed Sheet, Popover or Dialog must never leave focus on the
 * body, so a keyboard or screen-reader user keeps their place.
 */
export function returnFocus(
  opener: Focusable | null | undefined,
  fallback?: Focusable | null,
): void {
  if (opener?.isConnected) opener.focus();
  else fallback?.focus();
}
