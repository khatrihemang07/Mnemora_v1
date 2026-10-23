import type { RoutePath } from '../../lib/router';

export type SyncTab = 'devices' | 'conflicts' | 'activity';

/** Sync's pages, in order. Each has its own address (B6b), so a link lands on a tab and Back returns to the last one. */
export const SYNC_TABS: { value: SyncTab; label: string; path: RoutePath }[] = [
  { value: 'devices', label: 'Devices', path: '/sync' },
  { value: 'conflicts', label: 'Conflicts', path: '/sync/conflicts' },
  { value: 'activity', label: 'Activity', path: '/sync/activity' },
];

/** The tab an address belongs to; anything under `/sync` that is not a known page is Devices. */
export function tabForPath(pathname: string): SyncTab {
  // Compare (`/sync/conflicts/‹id›`) belongs to Conflicts.
  if (pathname.startsWith('/sync/conflicts/')) return 'conflicts';
  return SYNC_TABS.find((t) => t.path === pathname)?.value ?? 'devices';
}

/** The address of a tab. */
export function pathForTab(tab: SyncTab): RoutePath {
  return SYNC_TABS.find((t) => t.value === tab)?.path ?? '/sync';
}
