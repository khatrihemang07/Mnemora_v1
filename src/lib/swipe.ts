/** How far a finger travels before we decide which way a gesture goes. */
export const AXIS_LOCK_PX = 8;
/** A sideways swipe on a row commits at this distance (about a fifth of a 360px screen). */
export const ROW_COMMIT_PX = 72;
/** A sideways swipe on the title or segment bar must travel this far to change view... */
export const VIEW_SWIPE_PX = 60;
/** ...and stay this close to horizontal. */
export const VIEW_SWIPE_MAX_DY_PX = 45;
/** A sheet pulled down this far (or flicked) closes. */
export const SHEET_CLOSE_PX = 110;
const SHEET_FLICK_PX_PER_MS = 0.6;
const SHEET_FLICK_MIN_PX = 40;

export type Axis = 'x' | 'y';

/**
 * Which way a gesture is going, once the finger has left the dead zone: `x` when it is clearly
 * sideways, otherwise `y` (the browser scrolls). Null while it is still too short to tell.
 */
export function lockAxis(dx: number, dy: number, deadZone = AXIS_LOCK_PX): Axis | null {
  if (Math.hypot(dx, dy) < deadZone) return null;
  return Math.abs(dx) > Math.abs(dy) * 1.2 ? 'x' : 'y';
}

/** Which sides of a row a swipe may reveal. */
export interface SwipeSides {
  right: boolean;
  left: boolean;
}

/**
 * Where a row sits while a finger has dragged it by `dx`: free up to the commit distance, then
 * with growing resistance, and not at all towards a side that has no action.
 */
export function rowOffset(dx: number, sides: SwipeSides): number {
  if ((dx > 0 && !sides.right) || (dx < 0 && !sides.left)) return 0;
  const past = Math.abs(dx) - ROW_COMMIT_PX;
  if (past <= 0) return dx;
  return Math.sign(dx) * (ROW_COMMIT_PX + past * 0.35);
}

/** A flick: at least this far, at least this fast (px per ms), commits a row without the full distance. */
export const FLICK_MIN_PX = 40;
export const FLICK_PX_PER_MS = 0.6;

/**
 * What releasing a row at `dx` does. `ms` is how long the gesture lasted: a quick flick counts even
 * when it falls short of the commit distance.
 */
export function rowSwipeOutcome(dx: number, sides: SwipeSides, ms = 0): 'right' | 'left' | null {
  const flick = ms > 0 && Math.abs(dx) >= FLICK_MIN_PX && Math.abs(dx) / ms >= FLICK_PX_PER_MS;
  if ((dx >= ROW_COMMIT_PX || (flick && dx > 0)) && sides.right) return 'right';
  if ((dx <= -ROW_COMMIT_PX || (flick && dx < 0)) && sides.left) return 'left';
  return null;
}

/**
 * Whether a finished sideways gesture on the title or segment bar changes view: -1 for the
 * previous one (swiped right), 1 for the next (swiped left), 0 for neither.
 */
export function viewSwipeStep(dx: number, dy: number): -1 | 0 | 1 {
  if (Math.abs(dx) < VIEW_SWIPE_PX || Math.abs(dy) > VIEW_SWIPE_MAX_DY_PX) return 0;
  return dx < 0 ? 1 : -1;
}

/** Whether a sheet pulled down by `dy` in `ms` should close on release. */
export function sheetShouldClose(dy: number, ms: number): boolean {
  if (dy >= SHEET_CLOSE_PX) return true;
  return dy >= SHEET_FLICK_MIN_PX && ms > 0 && dy / ms >= SHEET_FLICK_PX_PER_MS;
}
