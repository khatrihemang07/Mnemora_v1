/**
 * Runs the shared rule fixtures (fixtures/sync-rules.json) against the browser mock. The Rust
 * commands run the same file in src-tauri/src/sync.rs (ADR 0009), plus the Device settings.
 */
import { describe, expect, it } from 'vitest';
import type { DeviceSettings } from './bindings';
import rules from './fixtures/sync-rules.json';
import { createSyncMock } from './mock-sync';

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
