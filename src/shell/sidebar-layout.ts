export const MIN_WIDTH = 200;
export const MAX_WIDTH = 360;
export const DEFAULT_WIDTH = 248;
export const RAIL_WIDTH = 60;
/** Dragging narrower than this collapses to the rail. */
export const SNAP_BELOW = 150;

export interface SidebarLayout {
  /** Expanded width, remembered while in rail mode. */
  width: number;
  rail: boolean;
}

export const clampWidth = (px: number) => Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, px));

/** Turns a dragged pixel width into a layout. Same rule from rail or expanded: past 150px expands. */
export function settleDrag(px: number, current: SidebarLayout): SidebarLayout {
  if (px < SNAP_BELOW) return { width: current.width, rail: true };
  return { width: clampWidth(px), rail: false };
}

export const toggleRail = (l: SidebarLayout): SidebarLayout => ({ ...l, rail: !l.rail });
export const resetLayout = (): SidebarLayout => ({ width: DEFAULT_WIDTH, rail: false });
export const shownWidth = (l: SidebarLayout) => (l.rail ? RAIL_WIDTH : l.width);
