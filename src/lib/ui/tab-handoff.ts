/**
 * Keyboard focus across a route change. When each tab is its own route the Tabs Component remounts,
 * so the focused tab disappears with the old page. A change made through Tabs is `note`d, and the
 * instance that mounts within `HANDOFF_MS` `claim`s it to focus its active tab.
 *
 * Arrow keys can outrun the route change: two changes made before the first page has mounted give
 * two mounts in a row, and each of them must get the focus (the second replaces the first, which
 * took it). So every change counts, and every mount claims one.
 */
export const HANDOFF_MS = 1500;

let pending = 0;
let until = 0;

/** Remembers that the tab just changed. */
export function noteTabChange(now = Date.now()): void {
  if (now > until) pending = 0;
  pending += 1;
  until = now + HANDOFF_MS;
}

/** True for each mount soon after a tab change, once per change. */
export function claimTabFocus(now = Date.now()): boolean {
  if (now > until || pending === 0) {
    pending = 0;
    return false;
  }
  pending -= 1;
  return true;
}
