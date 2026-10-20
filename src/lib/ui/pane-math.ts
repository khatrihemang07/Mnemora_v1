/** Pure pixel/percent maths for ResizablePanes (paneforge works in percent of the group). */

/** Ignore pane size reports this close (px) to the width we asked for; they are our own echo. */
export const ECHO_TOLERANCE_PX = 1.5;

export const pxToPercent = (px: number, groupWidth: number) =>
  groupWidth ? (px / groupWidth) * 100 : 0;
export const percentToPx = (percent: number, groupWidth: number) => (percent / 100) * groupWidth;

/** Pane limits in percent for a pixel range. */
export const paneLimits = (minPx: number, maxPx: number, groupWidth: number) => ({
  min: pxToPercent(minPx, groupWidth),
  max: pxToPercent(maxPx, groupWidth),
});

/** The user's dragged width in px, or null when the report is just an echo of `shown`. */
export function draggedWidth(percent: number, groupWidth: number, shown: number): number | null {
  const px = percentToPx(percent, groupWidth);
  return Math.abs(px - shown) < ECHO_TOLERANCE_PX ? null : px;
}
