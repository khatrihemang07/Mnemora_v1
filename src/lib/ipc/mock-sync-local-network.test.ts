/** The Local Network permission in the browser mock: the hook QA uses to simulate it being refused (D13, D15). */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Attention, DeviceDetail, DeviceSettings } from './bindings';
import { createSyncMock } from './mock-sync';
import { describe as describeLine, type Event } from './mock-sync-activity';
import { defaultPairingFlags } from './mock-sync-pairing';

const AIR = 'mock-air';

type View = {
  this_device: { platform: string; local_network_blocked: boolean; addresses: string[] };
  peers: { id: string; state: string }[];
};

function setup(sync: DeviceSettings['sync'] = { enabled: true, name: 'Alex’s Mac' }) {
  const device: DeviceSettings = { sync };
  const flags = defaultPairingFlags();
  const log: Event[] = [];
  const changed = vi.fn();
  const mock = createSyncMock(
    device,
    2,
    () => {},
    flags,
    changed,
    () => 0,
    (e) => log.push(e),
  );
  const call = <T>(cmd: string, a: Record<string, unknown> = {}) => mock.handle(cmd, a) as T;
  return { flags, log, changed, call };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('simulating the Local Network permission being refused', () => {
  it('is fine until the hook says otherwise', () => {
    const { call } = setup();
    const view = call<View>('sync_devices');
    expect(view.this_device.local_network_blocked).toBe(false);
    expect(view.peers.find((p) => p.id === AIR)?.state).toBe('not_nearby');
    expect(call<Attention>('sync_attention').local_network_blocked).toBe(false);
  });

  it('shows the banner flag, the row wording, the sheet state and the attention flag, then clears', () => {
    const { call, flags, changed } = setup();
    flags.localNetwork?.('blocked');
    expect(changed).toHaveBeenCalled();
    const view = call<View>('sync_devices');
    expect(view.this_device).toMatchObject({ platform: 'desktop', local_network_blocked: true });
    expect(view.peers.find((p) => p.id === AIR)?.state).toBe('blocked_by_local_network');
    expect(call<DeviceDetail>('sync_device', { id: AIR }).state).toBe('blocked_by_local_network');
    const attention = call<Attention>('sync_attention');
    expect(attention).toMatchObject({ local_network_blocked: true, needed: true });

    flags.localNetwork?.('ok');
    expect(call<View>('sync_devices').peers.find((p) => p.id === AIR)?.state).toBe('not_nearby');
    expect(call<Attention>('sync_attention').local_network_blocked).toBe(false);
  });

  it('words it for a phone when asked to', () => {
    const { call, flags } = setup();
    flags.localNetwork?.('blocked', 'phone');
    expect(call<View>('sync_devices').this_device.platform).toBe('phone');
  });

  it('is nothing while Sync is off', () => {
    const { call, flags } = setup({ enabled: false, name: 'Alex’s Mac' });
    flags.localNetwork?.('blocked');
    expect(call<View>('sync_devices').this_device.local_network_blocked).toBe(false);
  });

  it('mobile data is the stronger reason for a device that cannot be reached', () => {
    const { call, flags } = setup();
    flags.onMobile = true;
    flags.localNetwork?.('blocked');
    expect(call<View>('sync_devices').peers.find((p) => p.id === AIR)?.state).toBe(
      'paused_on_mobile_data',
    );
  });

  it('writes one Activity line when it starts, not again when it flickers', () => {
    const { flags, log } = setup();
    const words = () =>
      log.map((e) =>
        describeLine(e, 'mock-me')
          .text.map((p) => p.text)
          .join(''),
      );
    flags.localNetwork?.('blocked');
    flags.localNetwork?.('ok');
    flags.localNetwork?.('blocked');
    expect(words()).toEqual([
      'Can’t reach devices on the network: blocked by the Local Network setting',
    ]);
    vi.advanceTimersByTime(11 * 60_000);
    flags.localNetwork?.('ok');
    flags.localNetwork?.('blocked');
    expect(words()).toHaveLength(2);
  });

  it('answers the commands the buttons send', () => {
    const { call } = setup();
    expect(call('sync_open_local_network_settings')).toBeNull();
    expect(call('sync_local_network_os', { denied: true })).toBeNull();
  });
});
