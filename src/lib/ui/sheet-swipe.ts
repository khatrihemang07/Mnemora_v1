import { AXIS_LOCK_PX, sheetShouldClose } from '../swipe';

/** What a pull on a sheet must leave alone: fields and editors keep their own touch behaviour. */
const OWN_TOUCH =
  'input, textarea, select, [contenteditable="true"], .cm-editor, [data-no-sheet-swipe]';
/** The strip at the top of a sheet that always pulls it down, whatever is scrolled. */
const GRIP_PX = 32;
/** How long the sheet takes to leave or come back after a release. */
const SETTLE_MS = 160;

/**
 * Lets a bottom sheet be pulled down with a finger and closed by a long pull or a flick. A pull
 * starts on the top strip, or anywhere while the sheet is scrolled to its top (not on a field).
 * The sheet follows the finger (transform only); letting go either closes it or settles back.
 * Touch events, not pointer events, because the move must be cancellable to stop the page behind
 * from scrolling. Returns the cleanup.
 */
export function sheetSwipe(
  panel: HTMLElement,
  onclose: () => void,
  reduceMotion: () => boolean,
): () => void {
  let startY = 0;
  let startAt = 0;
  let pulling = false;
  let decided = false;

  const offset = (px: number) => {
    panel.style.transform = px > 0 ? `translateY(${px}px)` : '';
  };
  const settle = (to: string, then?: () => void) => {
    const ms = reduceMotion() ? 0 : SETTLE_MS;
    panel.style.transition = `transform ${ms}ms ease-out`;
    panel.style.transform = to;
    const done = () => {
      panel.style.transition = '';
      then?.();
    };
    if (ms === 0) done();
    else setTimeout(done, ms);
  };

  function onstart(e: TouchEvent) {
    const t = e.touches[0];
    if (!t || e.touches.length !== 1) return;
    const target = e.target as Element;
    const top = panel.getBoundingClientRect().top;
    const inGrip = t.clientY - top <= GRIP_PX;
    startY = t.clientY;
    startAt = performance.now();
    decided = false;
    pulling = inGrip || (panel.scrollTop <= 0 && !target.closest(OWN_TOUCH));
  }

  function onmove(e: TouchEvent) {
    const t = e.touches[0];
    if (!t || !pulling) return;
    const dy = t.clientY - startY;
    if (!decided) {
      if (Math.abs(dy) < AXIS_LOCK_PX) return;
      decided = true;
      // Upwards, or the sheet scrolled meanwhile: the content scrolls as usual.
      if (dy < 0 || panel.scrollTop > 0) {
        pulling = false;
        return;
      }
    }
    if (e.cancelable) e.preventDefault();
    offset(Math.max(0, dy));
  }

  function onend(e: TouchEvent) {
    if (!pulling || !decided) {
      pulling = false;
      return;
    }
    pulling = false;
    const dy = (e.changedTouches[0]?.clientY ?? startY) - startY;
    if (sheetShouldClose(dy, performance.now() - startAt)) settle('translateY(100%)', onclose);
    else settle('');
  }

  panel.addEventListener('touchstart', onstart, { passive: true });
  panel.addEventListener('touchmove', onmove, { passive: false });
  panel.addEventListener('touchend', onend, { passive: true });
  panel.addEventListener('touchcancel', onend, { passive: true });
  return () => {
    panel.removeEventListener('touchstart', onstart);
    panel.removeEventListener('touchmove', onmove);
    panel.removeEventListener('touchend', onend);
    panel.removeEventListener('touchcancel', onend);
  };
}
