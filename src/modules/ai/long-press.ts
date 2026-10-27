/** A point on the screen, in pixels. */
export interface Point {
  readonly x: number;
  readonly y: number;
}

/** How long a finger rests on a question before its Edit control shows (phones only). */
export const LONG_PRESS_MS = 500;
/** How far a finger may drift and still count as holding still, in pixels. */
const SLOP_PX = 10;

/** Whether a press has moved too far to be a long press: a scroll or a swipe, not a hold. */
export function pressMovedTooFar(from: Point, to: Point): boolean {
  return Math.hypot(to.x - from.x, to.y - from.y) > SLOP_PX;
}
