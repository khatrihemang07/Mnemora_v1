/** Loading placeholders stay hidden this long, so quick loads never flash a skeleton (N7-B). */
export const SKELETON_DELAY_MS = 300;

/** A timer for injection (`setTimer` deps): schedules `fn` and returns a function that cancels it. */
export const setTimer = (fn: () => void, ms: number): (() => void) => {
  const t = setTimeout(fn, ms);
  return () => clearTimeout(t);
};

/** Waits this long after the last change before writing a Device setting. */
export const PERSIST_DEBOUNCE_MS = 300;
