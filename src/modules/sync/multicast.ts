import type { AndroidBridge } from '../../lib/theme/system-bars';

/** The part of `document` that says whether the page is in front. */
export interface VisibilitySource {
  hidden: boolean;
  addEventListener(type: 'visibilitychange', listener: () => void): void;
  removeEventListener(type: 'visibilitychange', listener: () => void): void;
}

/**
 * Holds Android's Wi-Fi multicast lock while the page is visible, and lets go when it is hidden or
 * `stop` is called. Sync mounts this only while it is on. No-op without the bridge (desktop, browser).
 */
export function holdMulticastWhileVisible(
  bridge: AndroidBridge | undefined,
  doc: VisibilitySource = document,
): () => void {
  const hold = bridge?.holdMulticastLock;
  if (typeof hold !== 'function') return () => {};
  const apply = () => hold.call(bridge, !doc.hidden);
  apply();
  doc.addEventListener('visibilitychange', apply);
  return () => {
    doc.removeEventListener('visibilitychange', apply);
    hold.call(bridge, false);
  };
}
