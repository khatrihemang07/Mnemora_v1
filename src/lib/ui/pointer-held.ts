/** How many pointers (fingers, a mouse button) are down right now, from capture-phase listeners. */
let down = 0;

if (typeof window !== 'undefined') {
  const add = (type: string, fn: () => void) => window.addEventListener(type, fn, true);
  add('pointerdown', () => down++);
  const up = () => {
    down = Math.max(0, down - 1);
  };
  add('pointerup', up);
  add('pointercancel', up);
}

/** Whether a pointer is held down at this moment. */
export const pointerHeld = (): boolean => down > 0;

/**
 * Runs `fn` once nothing is held down any more (plus a beat, so the release's own click has passed),
 * or at once when nothing is. A menu opened by a long-press uses it to ignore the finger that is
 * still resting on the screen. Returns a cancel function.
 */
export function afterRelease(fn: () => void, beatMs = 120): () => void {
  if (!pointerHeld()) {
    fn();
    return () => {};
  }
  let timer: ReturnType<typeof setTimeout> | undefined;
  const release = () => {
    if (pointerHeld()) return;
    window.removeEventListener('pointerup', release, true);
    window.removeEventListener('pointercancel', release, true);
    timer = setTimeout(fn, beatMs);
  };
  // Our listeners are added after the tracker's own, so `down` is already updated when they run.
  window.addEventListener('pointerup', release, true);
  window.addEventListener('pointercancel', release, true);
  return () => {
    window.removeEventListener('pointerup', release, true);
    window.removeEventListener('pointercancel', release, true);
    clearTimeout(timer);
  };
}
