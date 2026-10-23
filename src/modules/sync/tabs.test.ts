import { describe, expect, it } from 'vitest';
import { buildRoutes } from '../../lib/router/routes';
import { syncManifest } from './manifest';
import { pathForTab, SYNC_TABS, tabForPath } from './tabs';

describe('tab routing', () => {
  it('gives every tab its own address', () => {
    expect(SYNC_TABS.map((t) => t.path)).toEqual(['/sync', '/sync/conflicts', '/sync/activity']);
  });

  it('reads the tab from the address', () => {
    expect(tabForPath('/sync')).toBe('devices');
    expect(tabForPath('/sync/conflicts')).toBe('conflicts');
    expect(tabForPath('/sync/activity')).toBe('activity');
  });

  it('keeps Compare under Conflicts', () => {
    expect(tabForPath('/sync/conflicts/abc')).toBe('conflicts');
  });

  it('falls back to Devices for an address it does not know', () => {
    expect(tabForPath('/sync/nowhere')).toBe('devices');
    expect(tabForPath('/')).toBe('devices');
  });

  it('round-trips every tab through its address', () => {
    for (const t of SYNC_TABS) expect(tabForPath(pathForTab(t.value))).toBe(t.value);
  });

  it('is registered as one route per tab, so Back returns to the previous tab', () => {
    const routes = buildRoutes([syncManifest]);
    expect(Object.keys(routes).sort()).toEqual(
      [...SYNC_TABS.map((t) => t.path), '/sync/conflicts/:id'].sort(),
    );
  });
});
