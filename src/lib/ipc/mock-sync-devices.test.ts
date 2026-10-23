/** The device sheet in the browser mock: detail, Rename, the typed Address, Check and Forget. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CheckResult, DeviceDetail, DeviceSettings } from './bindings';
import { createSyncMock } from './mock-sync';
import { defaultPairingFlags } from './mock-sync-pairing';

const PIXEL = 'mock-pixel';
const AIR = 'mock-air';

function setup() {
  const device: DeviceSettings = { sync: { enabled: true, name: 'Alex’s Mac' } };
  const flags = defaultPairingFlags();
  const changed = vi.fn();
  const mock = createSyncMock(device, 2, () => {}, flags, changed);
  const call = <T>(cmd: string, a: Record<string, unknown>) => mock.handle(cmd, a) as T;
  return { mock, flags, changed, call };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('the device sheet', () => {
  it('shows addresses with when they worked, versions and who paired it', () => {
    const { call } = setup();
    const d = call<DeviceDetail>('sync_device', { id: PIXEL });
    expect(d).toMatchObject({
      name: 'Pixel 8 (simulated)',
      app_version: '0.9.2',
      paired_by: 'Alex’s Mac',
    });
    expect(d.addresses.map((a) => a.address)).toEqual(['192.168.1.31:47311', '100.88.12.4:47311']);
    expect(d.data_versions.map((m) => m.module)).toContain('todo');
    expect(() => call('sync_device', { id: 'nobody' })).toThrow(
      expect.objectContaining({ code: 'not_found' }),
    );
  });
});

describe('rename', () => {
  it('is unique across the group ignoring case and accents, and 1 to 40 characters', () => {
    const { call, changed } = setup();
    for (const clash of ['macbook air (SIMULATED)', 'alex’s mac']) {
      expect(() => call('sync_device_rename', { id: PIXEL, name: clash })).toThrow(
        expect.objectContaining({ code: 'validation' }),
      );
    }
    expect(() => call('sync_device_rename', { id: PIXEL, name: '  ' })).toThrow();
    expect(() => call('sync_device_rename', { id: PIXEL, name: 'x'.repeat(41) })).toThrow();
    expect(call<string>('sync_device_rename', { id: PIXEL, name: '  Pixel 9 ' })).toBe('Pixel 9');
    expect(call<DeviceDetail>('sync_device', { id: PIXEL }).name).toBe('Pixel 9');
    expect(changed).toHaveBeenCalled();
  });
});

describe('the typed Address', () => {
  it('is stored as normalised, refused with the reason, and cleared by empty text', () => {
    const { call } = setup();
    expect(call('sync_device_address', { id: PIXEL, address: ' Pixel.Tail1234.TS.net ' })).toBe(
      'pixel.tail1234.ts.net',
    );
    expect(call<DeviceDetail>('sync_device', { id: PIXEL }).typed_address).toBe(
      'pixel.tail1234.ts.net',
    );
    expect(() => call('sync_device_address', { id: PIXEL, address: 'pixel 8' })).toThrow(
      expect.objectContaining({ message: 'That isn’t a host name or an IP address' }),
    );
    expect(call('sync_device_address', { id: PIXEL, address: '' })).toBeNull();
    expect(call<DeviceDetail>('sync_device', { id: PIXEL }).typed_address).toBeNull();
  });
});

describe('Check', () => {
  const check = (call: ReturnType<typeof setup>['call'], address: string) =>
    call<CheckResult>('sync_device_check', { id: PIXEL, address });

  it('reaches the device, or says why not', () => {
    const { call } = setup();
    expect(check(call, 'pixel.tail1234.ts.net')).toMatchObject({
      reached: true,
      kind: 'reached',
      message: 'Reached Pixel 8 (simulated) in 84 ms',
      ms: 84,
    });
    expect(check(call, 'pixel.timeout').kind).toBe('timed_out');
    expect(check(call, 'nowhere.example').kind).toBe('not_found');
    expect(check(call, 'closed.example').kind).toBe('unreachable');
    expect(check(call, 'wrong.example')).toMatchObject({ kind: 'wrong_device', reached: false });
    expect(check(call, 'unpaired.example').kind).toBe('not_paired');
    expect(check(call, 'not an address!')).toMatchObject({
      kind: 'invalid',
      message: 'That isn’t a host name or an IP address',
    });
  });

  it('can be forced from the console', () => {
    const { call, flags } = setup();
    flags.check = 'timed_out';
    expect(check(call, 'pixel.tail1234.ts.net').kind).toBe('timed_out');
  });
});

describe('Forget', () => {
  it('drops the device from the list and from the count', () => {
    const { call, mock, changed } = setup();
    call('sync_device_forget', { id: AIR });
    const peers = (mock.handle('sync_devices', {}) as { peers: { id: string }[] }).peers;
    expect(peers.map((p) => p.id)).toEqual([PIXEL]);
    expect((mock.handle('sync_get', {}) as { paired_devices: number }).paired_devices).toBe(1);
    expect(() => call('sync_device', { id: AIR })).toThrow();
    expect(changed).toHaveBeenCalled();
  });

  it('Sync now for one device runs a simulated sync with that device only', () => {
    const { call, mock } = setup();
    call('sync_device_sync_now', { id: PIXEL });
    const rows = (mock.handle('sync_devices', {}) as { peers: { id: string; state: string }[] })
      .peers;
    expect(rows.find((p) => p.id === PIXEL)?.state).toBe('syncing');
    expect(rows.find((p) => p.id === AIR)?.state).toBe('not_nearby');
  });
});
