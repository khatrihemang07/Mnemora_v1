/** How often the Devices page re-reads this device's addresses while it is visible. */
export const DEVICES_REFRESH_MS = 10_000;

/**
 * Calls `refresh` every `ms` while the page is visible, and when the window gets focus or becomes
 * visible again: a network (Wi-Fi, Tailscale) can come or go with no event from Sync.
 * Returns what stops it.
 */
export function refreshWhileVisible(
  refresh: () => void,
  ms = DEVICES_REFRESH_MS,
  doc: Document = document,
  win: Window = window,
): () => void {
  const timer = setInterval(() => {
    if (doc.visibilityState === 'visible') refresh();
  }, ms);
  const onVisible = () => {
    if (doc.visibilityState === 'visible') refresh();
  };
  win.addEventListener('focus', onVisible);
  doc.addEventListener('visibilitychange', onVisible);
  return () => {
    clearInterval(timer);
    win.removeEventListener('focus', onVisible);
    doc.removeEventListener('visibilitychange', onVisible);
  };
}
