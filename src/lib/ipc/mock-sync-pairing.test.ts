/**
 * The pairing flow in the browser mock: the shared rule fixtures, then each phase and each failure
 * the simulated second device can produce.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DeviceSettings, PairingView } from './bindings';
import rules from './fixtures/sync-rules.json';
import { createSyncMock } from './mock-sync';
import { defaultPairingFlags, normaliseCode, uniqueName } from './mock-sync-pairing';

describe('rules shared with Rust', () => {
  for (const c of rules.unique_name_cases) {
    it(`a device name: ${c.name}`, () => {
      expect(uniqueName(c.wanted, c.taken)).toBe(c.stored);
    });
  }
  for (const c of rules.code_cases) {
    it(`a typed code: ${c.name}`, () => {
      expect(normaliseCode(c.input)).toBe(c.code);
    });
  }
});

function setup(flags = defaultPairingFlags(), paired = 0) {
  const device: DeviceSettings = { sync: { enabled: true, name: 'Alex’s Mac' } };
  const views: PairingView[] = [];
  const mock = createSyncMock(device, paired, (v) => views.push(v), flags);
  const call = (cmd: string, a: Record<string, unknown> = {}) => mock.handle(cmd, a);
  const last = () => views[views.length - 1] as PairingView;
  const names = () =>
    (call('sync_devices') as { peers: { name: string }[] }).peers.map((p) => p.name);
  return { call, views, last, names, mock };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('showing a code', () => {
  it('shows 8 digits in two groups with a 2-minute window', () => {
    const { call, last } = setup();
    call('sync_pair_show');
    expect(last()).toMatchObject({ state: 'showing', life_ms: 120_000, tries_left: 3 });
    expect((last() as { code: string }).code).toMatch(/^\d{4} \d{4}$/);
  });

  it('the simulated phone then asks, Allow pairs, and both are listed', () => {
    const { call, last, names } = setup();
    call('sync_pair_show');
    vi.advanceTimersByTime(4000);
    expect(last()).toMatchObject({ state: 'asking', name: 'Pixel 8 (simulated)' });
    call('sync_pair_decide', { allow: true });
    expect(last().state).toBe('finishing');
    vi.advanceTimersByTime(1000);
    expect(last()).toMatchObject({ state: 'paired', name: 'Pixel 8 (simulated)', renamed: false });
    expect(names()).toEqual(['Pixel 8 (simulated)']);
    expect(call('sync_get')).toMatchObject({ paired_devices: 1 });
  });

  it('Deny closes without pairing', () => {
    const { call, last, names } = setup();
    call('sync_pair_show');
    vi.advanceTimersByTime(4000);
    call('sync_pair_decide', { allow: false });
    expect(last().state).toBe('idle');
    expect(names()).toEqual([]);
  });

  it('the code expires, and New code starts again with 3 tries', () => {
    const { call, last } = setup({ ...defaultPairingFlags(), pixel: 'never', lifeMs: 10_000 });
    call('sync_pair_show');
    vi.advanceTimersByTime(10_000);
    expect(last()).toEqual({ state: 'failed', error: { kind: 'code_expired', name: null } });
    call('sync_pair_new_code');
    expect(last()).toMatchObject({ state: 'showing', tries_left: 3 });
  });

  it('three wrong tries kill the code', () => {
    const { call, last } = setup({ ...defaultPairingFlags(), pixel: 'wrong' });
    call('sync_pair_show');
    vi.advanceTimersByTime(4000);
    expect(last()).toMatchObject({ state: 'showing', tries_left: 2 });
    vi.advanceTimersByTime(8000);
    expect(last()).toEqual({ state: 'failed', error: { kind: 'tries_used' } });
  });

  it('a phone that brings its group joins it too', () => {
    const { call, last, names } = setup({ ...defaultPairingFlags(), pixel: 'joins' });
    call('sync_pair_show');
    vi.advanceTimersByTime(4000);
    call('sync_pair_decide', { allow: true });
    vi.advanceTimersByTime(1000);
    expect(last()).toMatchObject({
      state: 'paired',
      also_joined: ['MacBook Air (simulated)', 'iPad (simulated)'],
    });
    expect(names()).toHaveLength(3);
  });

  it('cancel stops everything', () => {
    const { call, last } = setup();
    call('sync_pair_show');
    call('sync_pair_cancel');
    vi.advanceTimersByTime(200_000);
    expect(last().state).toBe('idle');
  });

  it('is refused while Sync is off', () => {
    const device: DeviceSettings = { sync: { enabled: false } };
    const mock = createSyncMock(device);
    expect(() => mock.handle('sync_pair_show', {})).toThrow(
      expect.objectContaining({ code: 'validation' }),
    );
  });
});

describe('typing a code', () => {
  const type = (s: ReturnType<typeof setup>, code: string, wait = 3000) => {
    s.call('sync_pair_enter', { code });
    expect(s.last().state).toBe('searching');
    vi.advanceTimersByTime(wait);
    return s.last();
  };

  it('refuses a code that is not 8 digits', () => {
    const s = setup();
    expect(() => s.call('sync_pair_enter', { code: '1234' })).toThrow(
      expect.objectContaining({ message: 'Type all 8 digits of the code' }),
    );
  });

  it('the right code waits for Allow and pairs', () => {
    const s = setup();
    s.call('sync_pair_enter', { code: '1234 5678' });
    vi.advanceTimersByTime(900);
    expect(s.last()).toMatchObject({ state: 'waiting', name: 'Pixel 8 (simulated)' });
    vi.advanceTimersByTime(1500);
    expect(s.last()).toMatchObject({ state: 'paired' });
  });

  it('a name that clashes becomes (2)', () => {
    const s = setup(defaultPairingFlags(), 1);
    expect(s.names()).toEqual(['Pixel 8 (simulated)']);
    const done = type(s, '90000000', 3000);
    expect(done).toMatchObject({ state: 'paired', name: 'Pixel 8 (simulated) (2)', renamed: true });
  });

  it('a wrong code counts down the tries and then dies', () => {
    const s = setup();
    expect(type(s, '11111111', 900)).toMatchObject({
      error: { kind: 'wrong_code', tries_left: 2 },
    });
    expect(type(s, '22222222', 900)).toMatchObject({
      error: { kind: 'wrong_code', tries_left: 1 },
    });
    expect(type(s, '33333333', 900)).toMatchObject({
      error: { kind: 'wrong_code', tries_left: 0 },
    });
  });

  it('once the tries are spent the lockout stays: every code is refused, the right one too (M7)', () => {
    const s = setup();
    for (const code of ['11111111', '22222222', '33333333']) type(s, code, 900);
    expect(type(s, '44444444', 900)).toMatchObject({ error: { kind: 'connection_lost' } });
    expect(type(s, '12345678', 900)).toMatchObject({ error: { kind: 'connection_lost' } });
    s.call('sync_pair_cancel');
    expect(type(s, '11111111', 900)).toMatchObject({
      error: { kind: 'wrong_code', tries_left: 2 },
    });
  });

  const failures: [string, string][] = [
    ['00000000', 'nothing_nearby'],
    ['20000000', 'code_expired'],
    ['40000000', 'different_environment'],
    ['50000000', 'version_mismatch'],
    ['50000001', 'version_mismatch'],
    ['50000002', 'version_mismatch'],
    ['60000000', 'connection_lost'],
    ['70000000', 'different_groups'],
    ['80000000', 'busy'],
  ];
  for (const [code, kind] of failures) {
    it(`${code} fails with ${kind}`, () => {
      expect(type(setup(), code, 900)).toMatchObject({ state: 'failed', error: { kind } });
    });
  }

  it('declined and cancelled arrive after the wait', () => {
    expect(type(setup(), '30000000', 3000)).toMatchObject({ error: { kind: 'declined' } });
    expect(type(setup(), '99999999', 3000)).toMatchObject({ error: { kind: 'cancelled' } });
  });

  it('a group joins with 91000000', () => {
    const s = setup();
    expect(type(s, '91000000', 3000)).toMatchObject({ state: 'paired' });
    expect(s.names()).toHaveLength(3);
  });
});

describe('the Devices tab', () => {
  it('lists this device with its addresses and the paired ones', () => {
    const { call } = setup(defaultPairingFlags(), 2);
    expect(call('sync_devices')).toMatchObject({
      this_device: { port: 47311, addresses: ['192.168.1.20', '100.101.4.7'] },
      peers: [{ name: 'Pixel 8 (simulated)' }, { name: 'MacBook Air (simulated)' }],
    });
  });

  it('leaving forgets them', () => {
    const { call, names } = setup(defaultPairingFlags(), 2);
    call('sync_leave');
    expect(names()).toEqual([]);
  });
});

describe('pairing by address', () => {
  const pairAt = (s: ReturnType<typeof setup>, address: string, code = '12345678') => {
    s.call('sync_pair_enter_at', { address, code });
    expect(s.last().state).toBe('searching');
    vi.advanceTimersByTime(900 + 1500);
    return s.last();
  };

  it('shows the route the address leads to, not always Wi‑Fi', () => {
    const routeAfter = (address: string) => {
      const s = setup();
      pairAt(s, address);
      return (s.call('sync_devices') as { peers: { route: string | null }[] }).peers[0]?.route;
    };
    expect(routeAfter('100.101.4.7')).toBe('tailscale');
    expect(routeAfter('mac.tail1234.ts.net')).toBe('tailscale');
    expect(routeAfter('192.168.1.40')).toBe('wifi');
  });

  it('refuses an address that breaks the rules and a code that is not 8 digits', () => {
    const s = setup();
    expect(() =>
      s.call('sync_pair_enter_at', { address: 'bad address!!', code: '12345678' }),
    ).toThrow(expect.objectContaining({ message: 'That isn’t a host name or an IP address' }));
    expect(() => s.call('sync_pair_enter_at', { address: 'pixel 8', code: '12345678' })).toThrow(
      expect.objectContaining({ message: 'That isn’t a host name or an IP address' }),
    );
    expect(() => s.call('sync_pair_enter_at', { address: 'mac.ts.net', code: '12' })).toThrow(
      expect.objectContaining({ message: 'Type all 8 digits of the code' }),
    );
  });

  it('pairs and keeps the address as the device’s typed Address', () => {
    const s = setup();
    expect(pairAt(s, ' Mac.Tail1234.ts.net ')).toMatchObject({ state: 'paired' });
    const id = (s.call('sync_devices') as { peers: { id: string }[] }).peers[0]?.id;
    expect(s.call('sync_device', { id })).toMatchObject({ typed_address: 'mac.tail1234.ts.net' });
  });

  it('says when the name is not found or nothing answers', () => {
    const s = setup();
    s.call('sync_pair_enter_at', { address: 'nowhere.ts.net', code: '12345678' });
    vi.advanceTimersByTime(900);
    expect(s.last()).toMatchObject({ state: 'failed', error: { kind: 'address_not_found' } });
    s.call('sync_pair_enter_at', { address: '100.64.0.9', code: '12345678' });
    vi.advanceTimersByTime(2400);
    expect(s.last().state).toBe('paired');
    const t = setup();
    t.call('sync_pair_enter_at', { address: 'closed.ts.net', code: '12345678' });
    vi.advanceTimersByTime(900);
    expect(t.last()).toMatchObject({ state: 'failed', error: { kind: 'address_unreachable' } });
  });

  it('the usual pairing errors still apply', () => {
    const s = setup();
    expect(pairAt(s, 'mac.ts.net', '11111111')).toMatchObject({
      state: 'failed',
      error: { kind: 'wrong_code', tries_left: 2 },
    });
  });
});

describe('the merge preview', () => {
  it('shows counts after the right code; Merge pairs', () => {
    const s = setup();
    s.call('sync_pair_enter', { code: '92000000' });
    vi.advanceTimersByTime(900 + 1350);
    expect(s.last()).toMatchObject({
      state: 'preview',
      name: 'Pixel 8 (simulated)',
      there: { entries: 214, tasks: 96, lists: 7 },
      here: { entries: 12, tasks: 30, lists: 4 },
      combine: ['Groceries', 'Work'],
    });
    expect(s.names()).toEqual([]);
    s.call('sync_pair_merge', { merge: true });
    expect(s.last().state).toBe('finishing');
    vi.advanceTimersByTime(900);
    expect(s.last().state).toBe('paired');
    expect(s.names()).toEqual(['Pixel 8 (simulated)']);
  });

  it('Cancel closes without pairing', () => {
    const s = setup();
    s.call('sync_pair_enter', { code: '92000000' });
    vi.advanceTimersByTime(3000);
    s.call('sync_pair_merge', { merge: false });
    expect(s.last().state).toBe('idle');
    vi.advanceTimersByTime(5000);
    expect(s.last().state).toBe('idle');
    expect(s.names()).toEqual([]);
  });

  it('a failed backup stops the merge', () => {
    const s = setup();
    s.call('sync_pair_enter', { code: '93000000' });
    vi.advanceTimersByTime(3000);
    s.call('sync_pair_merge', { merge: true });
    vi.advanceTimersByTime(900);
    expect(s.last()).toMatchObject({ state: 'failed', error: { kind: 'backup_failed' } });
    expect(s.names()).toEqual([]);
  });

  it('the showing device waits, then pairs when the phone confirms', () => {
    const s = setup({ ...defaultPairingFlags(), pixel: 'merge' });
    s.call('sync_pair_show');
    vi.advanceTimersByTime(4000);
    s.call('sync_pair_decide', { allow: true });
    expect(s.last()).toMatchObject({ state: 'confirming', name: 'Pixel 8 (simulated)' });
    vi.advanceTimersByTime(2700);
    expect(s.last().state).toBe('paired');
  });

  it('the showing device is told when the phone cancels, and keeps its code', () => {
    const s = setup({ ...defaultPairingFlags(), pixel: 'merge_cancel' });
    s.call('sync_pair_show');
    vi.advanceTimersByTime(4000);
    s.call('sync_pair_decide', { allow: true });
    vi.advanceTimersByTime(2700);
    expect(s.last()).toMatchObject({
      state: 'showing',
      notice: { kind: 'cancelled', name: 'Pixel 8 (simulated)' },
    });
    expect(s.names()).toEqual([]);
  });
});
