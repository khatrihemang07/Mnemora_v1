/** The device sheet in the browser mock: detail, Rename, the typed Address, Check and Forget. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CheckResult, DeviceDetail, DeviceSettings } from './bindings';
import { createSyncMock } from './mock-sync';
import { describe as describeLine, type Event } from './mock-sync-activity';
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
  it('is one value for the group: another member typing or clearing it shows here', () => {
    const { call, flags } = setup();
    flags.memberTypedAddress?.('100.64.0.7', PIXEL);
    expect(call<DeviceDetail>('sync_device', { id: PIXEL }).typed_address).toBe('100.64.0.7');
    flags.memberTypedAddress?.('');
    expect(call<DeviceDetail>('sync_device', { id: PIXEL }).typed_address).toBeNull();
  });

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

describe('Activity lines Forget, Rename and Leave write (M6)', () => {
  const lines = () => {
    const device: DeviceSettings = { sync: { enabled: true, name: 'Alex’s Mac' } };
    const log: Event[] = [];
    const mock = createSyncMock(
      device,
      2,
      () => {},
      defaultPairingFlags(),
      () => {},
      () => 0,
      (e) => log.push(e),
    );
    const words = () =>
      log.map((e) => {
        const row = describeLine(e, 'mock-me');
        return { kind: row.kind, text: row.text.map((p) => p.text).join(''), on: row.on };
      });
    return { mock, words };
  };

  it('Rename logs "‹old› renamed to ‹new›" and nothing when the name did not change', () => {
    const { mock, words } = lines();
    mock.handle('sync_device_rename', { id: PIXEL, name: 'Pixel 8 (simulated)' });
    expect(words()).toEqual([]);
    mock.handle('sync_device_rename', { id: PIXEL, name: 'Pixel 9' });
    expect(words()).toEqual([
      { kind: 'renamed', text: 'Pixel 8 (simulated) renamed to Pixel 9', on: null },
    ]);
  });

  it('Renaming this device logs "‹old› renamed to ‹new›" when a group exists (R1)', () => {
    const { mock, words } = lines();
    mock.handle('sync_rename', { name: 'Alex’s Mac' });
    expect(words()).toEqual([]);
    mock.handle('sync_rename', { name: 'Studio Mac' });
    expect(words()).toEqual([
      { kind: 'renamed', text: 'Alex’s Mac renamed to Studio Mac', on: null },
    ]);
  });

  it('Renaming this device with nobody paired logs nothing', () => {
    const log: Event[] = [];
    const mock = createSyncMock(
      { sync: { enabled: true, name: 'Alex’s Mac' } },
      0,
      () => {},
      defaultPairingFlags(),
      () => {},
      () => 0,
      (e) => log.push(e),
    );
    mock.handle('sync_rename', { name: 'Studio Mac' });
    expect(log).toEqual([]);
  });

  it('Forget logs "Forgot ‹name›", by the name it had', () => {
    const { mock, words } = lines();
    mock.handle('sync_device_forget', { id: AIR });
    expect(words()).toEqual([{ kind: 'forgot', text: 'Forgot MacBook Air (simulated)', on: null }]);
  });

  it('Leave logs "Left the group" once, and not with nobody paired', () => {
    const { mock, words } = lines();
    mock.handle('sync_leave', {});
    mock.handle('sync_leave', {});
    expect(words()).toEqual([{ kind: 'left', text: 'Left the group', on: null }]);
  });
});

describe('on mobile data with "Allow over mobile data" off', () => {
  type Rows = {
    this_device: { paused_on_mobile_data: boolean };
    peers: { id: string; state: string }[];
  };

  it('a device that cannot be reached reads Paused on mobile data, in the list and the sheet (P1)', () => {
    const { call, flags } = setup();
    flags.onMobile = true;
    const view = call<Rows>('sync_devices', {});
    expect(view.this_device.paused_on_mobile_data).toBe(true);
    expect(view.peers.find((p) => p.id === AIR)?.state).toBe('paused_on_mobile_data');
    expect(call<DeviceDetail>('sync_device', { id: AIR }).state).toBe('paused_on_mobile_data');
    // Allowing mobile data brings the plain state back.
    call('sync_set_option', { option: 'mobile_data', enabled: true });
    expect(call<Rows>('sync_devices', {}).peers.find((p) => p.id === AIR)?.state).toBe(
      'not_nearby',
    );
  });

  it('refuses every way of pairing up front and says why (P4)', () => {
    const { call, flags } = setup();
    flags.onMobile = true;
    const why = expect.objectContaining({
      code: 'validation',
      message: expect.stringContaining('Pairing is paused on mobile data'),
    });
    expect(() => call('sync_pair_show', {})).toThrow(why);
    expect(() => call('sync_pair_enter', { code: '12345678' })).toThrow(why);
    expect(() => call('sync_pair_enter_at', { address: 'mac.ts.net', code: '12345678' })).toThrow(
      why,
    );
    call('sync_set_option', { option: 'mobile_data', enabled: true });
    expect(() => call('sync_pair_show', {})).not.toThrow();
  });
});
