import { describe, expect, it } from 'vitest';
import type { PeerRow } from '../../lib/ipc/bindings';
import {
  absoluteTime,
  countsText,
  progressText,
  routeWord,
  rowParts,
  STATUS_WORDS,
  statusWord,
  syncedText,
} from './peer-status';

const now = new Date(2026, 9, 23, 12, 0, 0);
const MIN = 60_000;

const peer = (over: Partial<PeerRow> = {}): PeerRow => ({
  id: 'p',
  name: 'Pixel 8',
  platform: 'phone',
  paired_at: 0,
  state: 'up_to_date',
  last_seen_at: null,
  last_synced_at: now.getTime() - 2 * MIN,
  last_in: 12,
  last_out: 3,
  route: 'wifi',
  progress: null,
  pause: null,
  ...over,
});

describe('status words', () => {
  it('are the plain ones', () => {
    expect(Object.values(STATUS_WORDS)).toEqual([
      'Up to date',
      'Syncing…',
      'Not nearby',
      'Paused',
      'Removed',
    ]);
    expect(statusWord('not_nearby')).toBe('Not nearby');
  });

  it('name the route', () => {
    expect(routeWord('wifi')).toBe('Wi‑Fi');
    expect(routeWord('tailscale')).toBe('Tailscale');
  });
});

describe('the progress line', () => {
  it('reads "120 of 340 changes"', () => {
    expect(progressText({ done: 120, total: 340 })).toBe('120 of 340 changes');
  });

  it('is empty until the size is known', () => {
    expect(progressText(null)).toBe('');
    expect(progressText({ done: 0, total: 0 })).toBe('');
  });

  it('never counts past the total and says "change" for one', () => {
    expect(progressText({ done: 5, total: 3 })).toBe('3 of 3 changes');
    expect(progressText({ done: 0, total: 1 })).toBe('0 of 1 change');
  });
});

describe('last session counts', () => {
  it('read "12 in · 3 out"', () => {
    expect(countsText(peer())).toBe('12 in · 3 out');
    expect(countsText(peer({ last_in: null, last_out: 4 }))).toBe('0 in · 4 out');
  });

  it('are absent before anything moved', () => {
    expect(countsText(peer({ last_in: null, last_out: null }))).toBeNull();
  });
});

describe('relative times', () => {
  it('read "synced 2 min ago"', () => {
    expect(syncedText(peer(), now)).toBe('synced 2 min ago');
  });

  it('switch to a date after a week, and say so when it never happened', () => {
    const old = now.getTime() - 9 * 24 * 60 * MIN;
    expect(syncedText(peer({ last_synced_at: old }), now)).toBe('synced Wed 14 Oct');
    expect(syncedText(peer({ last_synced_at: null }), now)).toBe('not synced yet');
  });

  it('a paused device says "last synced"', () => {
    expect(syncedText(peer({ state: 'paused' }), now)).toBe('last synced 2 min ago');
  });
});

describe('the row line', () => {
  it('up to date: status, when, counts, route', () => {
    expect(rowParts(peer(), now)).toEqual([
      'Up to date',
      'synced 2 min ago',
      '12 in · 3 out',
      'Wi‑Fi',
    ]);
  });

  it('not nearby drops the counts', () => {
    expect(rowParts(peer({ state: 'not_nearby', route: 'tailscale' }), now)).toEqual([
      'Not nearby',
      'synced 2 min ago',
      'Tailscale',
    ]);
  });

  it('syncing reads "Syncing… 120 of 340 changes" and nothing else', () => {
    const row = peer({ state: 'syncing', progress: { done: 120, total: 340 } });
    expect(rowParts(row, now).join(' ')).toBe('Syncing… 120 of 340 changes');
  });

  it('syncing without a size yet is just "Syncing…"', () => {
    expect(rowParts(peer({ state: 'syncing' }), now)).toEqual(['Syncing…']);
  });
});

describe('absoluteTime', () => {
  it('uses the app’s day style and the Time format Preference', () => {
    const at = new Date(2026, 9, 14, 15, 5).getTime();
    expect(absoluteTime(at, '24h', now)).toBe('Wed 14 Oct, 15:05');
    expect(absoluteTime(at, '12h', now)).toBe('Wed 14 Oct, 3:05 PM');
  });
});
