/** Loading placeholders stay hidden this long, so quick loads never flash a skeleton (N7-B). */
export const SKELETON_DELAY_MS = 300;

/** A timer for injection (`setTimer` deps): schedules `fn` and returns a function that cancels it. */
export const setTimer = (fn: () => void, ms: number): (() => void) => {
  const t = setTimeout(fn, ms);
  return () => clearTimeout(t);
};

/** Waits this long after the last change before writing a Device setting. */
export const PERSIST_DEBOUNCE_MS = 300;

/** The hover that opens the icon rail's flyout, like a rail tooltip (N17-A). */
export const RAIL_FLYOUT_DELAY_MS = 600;
/** How long the flyout waits after the pointer leaves it and its icon before closing. */
export const RAIL_FLYOUT_LEAVE_MS = 250;
