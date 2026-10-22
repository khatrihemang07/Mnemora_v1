/**
 * Keyboard focus across a route change. When each tab is its own route the Tabs Component remounts,
 * so the focused tab disappears with the old page. A change made through Tabs is `note`d, and the
 * instance that mounts within `HANDOFF_MS` `claim`s it to focus its active tab.
 */
export const HANDOFF_MS = 1500;

let until = 0;

/** Remembers that the tab just changed. */
export function noteTabChange(now = Date.now()): void {
  until = now + HANDOFF_MS;
}

/** True once for a mount soon after a tab change. */
export function claimTabFocus(now = Date.now()): boolean {
  const fresh = now <= until;
  until = 0;
  return fresh;
}
