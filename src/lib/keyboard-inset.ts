/** Below this the visual viewport only changed for some other reason (browser bars, zoom). */
export const KEYBOARD_MIN_INSET = 80;

/**
 * How much of the layout viewport the on-screen keyboard covers, from the visual viewport. With
 * edge-to-edge windows the keyboard overlays the page, so the layout does not shrink by itself.
 */
export function keyboardInset(innerHeight: number, vvHeight: number, vvOffsetTop: number): number {
  return Math.max(0, Math.round(innerHeight - vvHeight - vvOffsetTop));
}

export const isKeyboardOpen = (inset: number): boolean => inset >= KEYBOARD_MIN_INSET;
