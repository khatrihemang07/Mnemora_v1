import { describe, expect, it } from 'vitest';
import type { Attention } from '../../lib/ipc/bindings';
import { badgeOf, homeCardLine, newConflictsText, paletteHint, reviewPath } from './attention';

const calm: Attention = {
  conflicts: 0,
  paused: [],
  unseen: [],
  local_network_blocked: false,
  last_synced: null,
  needed: false,
};
const NOW = new Date(2026, 9, 21, 10, 0);
const synced = (minutesAgo: number) => ({
  device: 'Pixel 8',
  at: NOW.getTime() - minutesAgo * 60_000,
});

describe('the Local Network setting blocking Sync (D13)', () => {
  const blocked: Attention = { ...calm, local_network_blocked: true, needed: true };

  it('is an amber exclamation mark in the badge, like a pause, and outranks the count', () => {
    expect(badgeOf(blocked)).toMatchObject({ text: '!', tone: 'warn' });
    expect(badgeOf({ ...blocked, conflicts: 3 })).toMatchObject({ text: '!', tone: 'warn' });
  });

  it('is a clause on the Home card', () => {
    expect(homeCardLine(blocked, NOW)).toBe('Blocked by the Local Network setting');
    expect(homeCardLine({ ...blocked, conflicts: 2 }, NOW)).toBe(
      '2 conflicts to review · Blocked by the Local Network setting',
    );
  });

  it('leads to the Devices page, where the banner is', () => {
    expect(reviewPath(blocked)).toBe('/sync');
  });
});

describe('the sidebar badge', () => {
  it('shows nothing while all is fine, and before it knows', () => {
    expect(badgeOf(calm)).toBeNull();
    expect(badgeOf(undefined)).toBeNull();
  });

  it('counts the conflicts that need you, in Sync’s colour', () => {
    expect(badgeOf({ ...calm, conflicts: 2 })).toMatchObject({ count: 2, tone: 'sync' });
  });

  it('is an amber exclamation mark while sync is paused, whatever else waits', () => {
    expect(badgeOf({ ...calm, conflicts: 2, paused: ['Pixel 8'] })).toMatchObject({
      text: '!',
      tone: 'warn',
    });
  });

  it('says what it is to a screen reader', () => {
    expect(badgeOf({ ...calm, conflicts: 1 })?.label).toBe('1 sync conflict needs you');
    expect(badgeOf({ ...calm, conflicts: 3 })?.label).toBe('3 sync conflicts need you');
  });
});

describe('the Home card line', () => {
  it('is the prototype’s line for waiting conflicts', () => {
    expect(homeCardLine({ ...calm, conflicts: 2, needed: true, last_synced: synced(2) }, NOW)).toBe(
      '2 conflicts to review · Pixel 8 synced 2 min ago',
    );
    expect(homeCardLine({ ...calm, conflicts: 1, needed: true }, NOW)).toBe('1 conflict to review');
  });

  it('names each issue, one clause apiece', () => {
    expect(
      homeCardLine(
        {
          conflicts: 0,
          paused: ['Pixel 8', 'Air'],
          unseen: [{ name: 'iPad', days: 16 }],
          local_network_blocked: false,
          last_synced: null,
          needed: true,
        },
        NOW,
      ),
    ).toBe('Paused with Pixel 8 and Air · iPad not seen for 16 days');
  });

  it('does not add a synced clause for a device an earlier clause is about', () => {
    expect(
      homeCardLine(
        {
          ...calm,
          unseen: [{ name: 'Pixel 8', days: 16 }],
          last_synced: synced(3),
          needed: true,
        },
        NOW,
      ),
    ).toBe('Pixel 8 not seen for 16 days');
    expect(
      homeCardLine({ ...calm, paused: ['Pixel 8'], last_synced: synced(3), needed: true }, NOW),
    ).toBe('Paused with Pixel 8');
    // Another device that did sync still gets its clause.
    expect(
      homeCardLine({ ...calm, paused: ['Air'], last_synced: synced(3), needed: true }, NOW),
    ).toBe('Paused with Air · Pixel 8 synced 3 min ago');
  });

  it('sends Review to the conflicts when some wait, else to the devices', () => {
    expect(reviewPath({ ...calm, conflicts: 1 })).toBe('/sync/conflicts');
    expect(reviewPath({ ...calm, paused: ['Pixel 8'] })).toBe('/sync');
  });
});

describe('palette and toast', () => {
  it('hints how many conflicts wait, and nothing at zero', () => {
    expect(paletteHint({ ...calm, conflicts: 2 })).toBe('2 waiting');
    expect(paletteHint(calm)).toBeNull();
    expect(paletteHint(undefined)).toBeNull();
  });

  it('words the toast for new conflicts', () => {
    expect(newConflictsText(1, 'Pixel 8')).toBe('1 sync conflict from Pixel 8');
    expect(newConflictsText(3, null)).toBe('3 sync conflicts');
  });
});

describe('the manifest', () => {
  it('offers the three palette actions, with the conflicts hint', async () => {
    const { syncManifest } = await import('./manifest');
    expect(syncManifest.paletteActions?.map((a) => a.label)).toEqual([
      'Sync now',
      'Pair a device',
      'Open Sync conflicts',
    ]);
  });

  it('keeps the badge off the phone and the Home card on offer', async () => {
    const { syncManifest } = await import('./manifest');
    expect(syncManifest.phoneBadge).toBe(false);
    expect(syncManifest.useBadge).toBeDefined();
    expect(syncManifest.homeCard).toBeDefined();
  });
});
