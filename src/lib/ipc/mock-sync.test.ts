/**
 * Runs the shared rule fixtures (fixtures/sync-rules.json) against the browser mock. The Rust
 * commands run the same file in src-tauri/src/sync.rs (ADR 0009), plus the Device settings.
 */
import { describe, expect, it } from 'vitest';
import type { DeviceSettings } from './bindings';
import rules from './fixtures/sync-rules.json';
import { attentionOf, createSyncMock } from './mock-sync';

const fresh = (paired = 0) => {
  const device: DeviceSettings = {};
  return { device, mock: createSyncMock(device, paired) };
};

describe('device name rules (shared with Rust)', () => {
  for (const c of rules.name_cases) {
    it(c.name, () => {
      const { mock } = fresh();
      if ('error' in c && c.error) {
        expect(() => mock.handle('sync_rename', { name: c.input })).toThrow(
          expect.objectContaining(c.error),
        );
      } else {
        mock.handle('sync_rename', { name: c.input });
        expect((mock.handle('sync_get', {}) as { name: string }).name).toBe(c.saved);
      }
    });
  }

  it('a refused name changes nothing', () => {
    const { mock } = fresh();
    mock.handle('sync_rename', { name: 'Pixel 8' });
    expect(() => mock.handle('sync_rename', { name: '' })).toThrow();
    expect((mock.handle('sync_get', {}) as { name: string }).name).toBe('Pixel 8');
  });
});

describe('renaming this device in a group', () => {
  it('refuses a name a paired device has, ignoring case and accents, saying so as for a paired one', () => {
    const { mock } = fresh(1);
    mock.handle('sync_rename', { name: 'Studio Mac' });
    for (const clash of ['pixel 8 (simulated)', ' PÍXEL 8 (SIMULATED) ']) {
      expect(() => mock.handle('sync_rename', { name: clash })).toThrow(
        expect.objectContaining({ message: 'Another of your devices already has that name' }),
      );
    }
    expect((mock.handle('sync_get', {}) as { name: string }).name).toBe('Studio Mac');
  });
});

describe('Device settings', () => {
  it('Sync starts off, with Rust’s defaults', () => {
    const { mock } = fresh();
    expect(mock.handle('sync_get', {})).toMatchObject({
      enabled: false,
      automatic: true,
      mobile_data: false,
      visible: true,
      paired_devices: 0,
    });
  });

  it('turning it on names the device once and keeps the name across off and on', () => {
    const { device, mock } = fresh();
    mock.handle('sync_set_enabled', { enabled: true });
    expect(device.sync?.enabled).toBe(true);
    expect(device.sync?.name).toBeTruthy();
    mock.handle('sync_rename', { name: 'Studio Mac' });
    mock.handle('sync_set_enabled', { enabled: false });
    mock.handle('sync_set_enabled', { enabled: true });
    expect(device.sync?.name).toBe('Studio Mac');
  });

  it('saves each option in the device file', () => {
    const { device, mock } = fresh();
    mock.handle('sync_set_option', { option: 'automatic', enabled: false });
    mock.handle('sync_set_option', { option: 'mobile_data', enabled: true });
    mock.handle('sync_set_option', { option: 'visible', enabled: false });
    expect(device.sync).toMatchObject({ automatic: false, mobile_data: true, visible: false });
  });

  it('leaving forgets the paired devices', () => {
    const { mock } = fresh(2);
    expect((mock.handle('sync_get', {}) as { paired_devices: number }).paired_devices).toBe(2);
    mock.handle('sync_leave', {});
    expect((mock.handle('sync_get', {}) as { paired_devices: number }).paired_devices).toBe(0);
  });
});

describe('what needs attention (mirrors crates/sync/src/attention.rs)', () => {
  const NOW = 100 * 86_400_000;
  const peer = (state: string, seen: number | null, synced: number | null = null) =>
    ({
      id: 'p',
      name: 'Pixel 8',
      platform: 'phone',
      paired_at: 0,
      state,
      last_seen_at: seen,
      last_synced_at: synced,
    }) as unknown as Parameters<typeof attentionOf>[1][number];

  it('is calm while all is fine', () => {
    expect(attentionOf(0, [peer('up_to_date', NOW, NOW)], NOW).needed).toBe(false);
  });

  it('shows for each of the three conditions alone', () => {
    expect(attentionOf(2, [], NOW).needed).toBe(true);
    expect(attentionOf(0, [peer('paused', NOW)], NOW)).toMatchObject({
      needed: true,
      paused: ['Pixel 8'],
    });
    expect(attentionOf(0, [peer('not_nearby', NOW - 14 * 86_400_000)], NOW).needed).toBe(true);
    expect(attentionOf(0, [peer('not_nearby', NOW - 13 * 86_400_000)], NOW).needed).toBe(false);
  });

  it('reads the mock device’s state: conflicts wait until they are settled', () => {
    const device: DeviceSettings = {};
    let open = 2;
    const mock = createSyncMock(device, 1, undefined, undefined, undefined, () => open);
    expect(mock.handle('sync_attention', {})).toMatchObject({ conflicts: 2, needed: true });
    open = 0;
    expect(mock.handle('sync_attention', {})).toMatchObject({ conflicts: 0, needed: false });
  });
});

describe('the new-conflicts choice', () => {
  it('is Quietly until it is set, per device', () => {
    const { mock } = fresh();
    expect(mock.handle('sync_get', {})).toMatchObject({ toast_conflicts: false });
    mock.handle('sync_set_option', { option: 'toast_conflicts', enabled: true });
    expect(mock.handle('sync_get', {})).toMatchObject({ toast_conflicts: true });
  });
});
