/** The simulated Pixel 8's status changes in the browser mock. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DeviceSettings } from './bindings';
import { createSyncMock } from './mock-sync';
import { defaultPairingFlags } from './mock-sync-pairing';

function setup(paired = 2) {
  const device: DeviceSettings = { sync: { enabled: true, name: 'Alex’s Mac' } };
  const changed = vi.fn();
  const mock = createSyncMock(device, paired, () => {}, undefined, changed);
  const peers = () =>
    (mock.handle('sync_devices', {}) as { peers: { name: string; state: string }[] }).peers;
  const row = (name: string) => {
    const found = (
      mock.handle('sync_devices', {}) as {
        peers: Record<string, unknown>[];
      }
    ).peers.find((p) => p.name === name);
    return found as Record<string, unknown>;
  };
  return { mock, changed, peers, row };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('paired devices in the mock', () => {
  it('start like the prototype: one here, one away on Tailscale, the reachable one first', () => {
    const { peers, row } = setup();
    expect(peers().map((p) => [p.name, p.state])).toEqual([
      ['Pixel 8 (simulated)', 'up_to_date'],
      ['MacBook Air (simulated)', 'not_nearby'],
    ]);
    expect(row('Pixel 8 (simulated)')).toMatchObject({ last_in: 12, last_out: 3, route: 'wifi' });
    expect(row('MacBook Air (simulated)')).toMatchObject({ route: 'tailscale' });
  });

  it('Sync now runs a simulated sync with progress, then settles with counts', () => {
    const { mock, changed, row } = setup();
    mock.handle('sync_now', {});
    expect(row('Pixel 8 (simulated)')).toMatchObject({
      state: 'syncing',
      progress: { done: 0, total: 340 },
    });
    vi.advanceTimersByTime(350 * 2);
    expect(row('Pixel 8 (simulated)').progress).toMatchObject({ done: 120, total: 340 });
    vi.advanceTimersByTime(350 * 6);
    expect(row('Pixel 8 (simulated)')).toMatchObject({ state: 'up_to_date', progress: null });
    expect(changed).toHaveBeenCalled();
    expect(row('MacBook Air (simulated)').state).toBe('not_nearby');
  });

  it('does nothing while Sync is off', () => {
    const device: DeviceSettings = { sync: { enabled: false } };
    const mock = createSyncMock(device, 2);
    mock.handle('sync_now', {});
    const rows = (mock.handle('sync_devices', {}) as { peers: { state: string }[] }).peers;
    expect(rows.every((p) => p.state !== 'syncing')).toBe(true);
  });

  it('the console helper puts a device in a state', () => {
    const device: DeviceSettings = { sync: { enabled: true } };
    const flags = { pixel: 'right', delayMs: 0, lifeMs: 1, stepMs: 1 } as never;
    const mock = createSyncMock(device, 2, () => {}, flags);
    (flags as { setPeer: (s: string) => void }).setPeer('syncing');
    const rows = (mock.handle('sync_devices', {}) as { peers: Record<string, unknown>[] }).peers;
    expect(rows[0]).toMatchObject({ state: 'syncing', progress: { done: 120, total: 340 } });
  });

  it('leaving the group drops the devices', () => {
    const { mock, peers } = setup();
    mock.handle('sync_leave', {});
    expect(peers()).toEqual([]);
  });
});

describe('paused devices in the mock', () => {
  it('show each reason of the paused banner and clear it again', () => {
    const flags = defaultPairingFlags();
    const mock = createSyncMock({ sync: { enabled: true } }, 2, () => {}, flags);
    const pixel = () =>
      (
        mock.handle('sync_devices', {}) as {
          peers: { name: string; state: string; pause: unknown }[];
        }
      ).peers.find((p) => p.name === 'Pixel 8 (simulated)');
    flags.setPeer?.('paused');
    expect(pixel()).toMatchObject({
      state: 'paused',
      pause: { kind: 'update', older_there: ['todo'] },
    });
    flags.setPeer?.('paused', undefined, 0, 'clock');
    expect(pixel()?.pause).toMatchObject({ kind: 'clock', here: false });
    flags.setPeer?.('paused', undefined, 0, 'clock_me');
    expect(pixel()?.pause).toMatchObject({ kind: 'clock', here: true });
    flags.setPeer?.('paused', undefined, 0, 'me');
    expect(pixel()?.pause).toMatchObject({ kind: 'update', older_here: ['todo'] });
    flags.setPeer?.('up_to_date');
    expect(pixel()).toMatchObject({ state: 'up_to_date', pause: null });
  });
});
