import { describe, expect, it, vi } from 'vitest';
import type { AndroidBridge } from '../../lib/theme/system-bars';
import {
  askLocalNetworkOnEnable,
  localNetworkBanner,
  pairByAddressHint,
  reportLocalNetworkPermission,
  WELCOME_LINE,
} from './local-network';

describe('the banner', () => {
  it('is not there while nothing is blocked, or before it is known', () => {
    expect(localNetworkBanner(undefined)).toBeNull();
    expect(localNetworkBanner({ platform: 'desktop', local_network_blocked: false })).toBeNull();
  });

  it('words the Mac case with the pane to open (P14)', () => {
    expect(localNetworkBanner({ platform: 'desktop', local_network_blocked: true })).toEqual({
      title: 'This Mac can’t reach devices on your network.',
      body: 'Allow Mnemora under System Settings › Privacy & Security › Local Network.',
      button: 'Open Settings',
    });
  });

  it('words the phone case for the app’s settings (P16)', () => {
    const phone = localNetworkBanner({ platform: 'phone', local_network_blocked: true });
    expect(phone?.title).toBe('This phone can’t reach devices on your network.');
    expect(phone?.button).toBe('Open Settings');
  });
});

describe('the Pair sheet’s address line on a Mac (P15)', () => {
  it('names the first address, the Wi-Fi one', () => {
    expect(
      pairByAddressHint({ platform: 'desktop', addresses: ['192.168.1.20', '100.101.4.7'] }),
    ).toEqual({ before: 'Or on your phone: Pair by address…', address: '192.168.1.20' });
  });

  it('is not shown without an address, or on a phone', () => {
    expect(pairByAddressHint({ platform: 'desktop', addresses: [] })).toBeNull();
    expect(pairByAddressHint({ platform: 'phone', addresses: ['192.168.1.9'] })).toBeNull();
    expect(pairByAddressHint(undefined)).toBeNull();
  });
});

describe('the Welcome page line (P13)', () => {
  it('says macOS will ask', () => {
    expect(WELCOME_LINE).toBe(
      'Mnemora will ask to find devices on your local network. Allow it so your devices can find each other.',
    );
  });
});

describe('asking Android for the permission', () => {
  it('asks when Sync is turned on, without opening settings', () => {
    const bridge: AndroidBridge = { requestLocalNetwork: vi.fn() };
    askLocalNetworkOnEnable(bridge);
    expect(bridge.requestLocalNetwork).toHaveBeenCalledWith(false);
  });

  it('does nothing without the bridge (desktop, browser)', () => {
    expect(() => askLocalNetworkOnEnable({})).not.toThrow();
  });
});

describe('reporting the permission to Rust', () => {
  function sources(hidden = false) {
    const listeners = new Map<string, () => void>();
    const target = {
      addEventListener: (type: string, fn: () => void) => listeners.set(type, fn),
      removeEventListener: (type: string) => listeners.delete(type),
    };
    return {
      listeners,
      src: {
        win: target as never,
        doc: Object.assign(target, { hidden }) as never,
      },
    };
  }

  it('says nothing and listens to nothing where the permission has no bridge', () => {
    const report = vi.fn();
    const { src, listeners } = sources();
    reportLocalNetworkPermission(undefined, report, src)();
    expect(report).not.toHaveBeenCalled();
    expect(listeners.size).toBe(0);
  });

  it('reports now, when the app is back and when the dialog is answered, and clears on stop', () => {
    let state = 'denied';
    const bridge: AndroidBridge = { localNetworkState: () => state };
    const report = vi.fn();
    const { src, listeners } = sources();
    const stop = reportLocalNetworkPermission(bridge, report, src);
    expect(report).toHaveBeenLastCalledWith(true);
    state = 'granted';
    listeners.get('mnemora-local-network')?.();
    expect(report).toHaveBeenLastCalledWith(false);
    state = 'denied';
    listeners.get('visibilitychange')?.();
    expect(report).toHaveBeenLastCalledWith(true);
    stop();
    expect(report).toHaveBeenLastCalledWith(false);
    expect(listeners.size).toBe(0);
  });

  it('counts "unneeded" (before Android 17) as not denied', () => {
    const report = vi.fn();
    reportLocalNetworkPermission({ localNetworkState: () => 'unneeded' }, report, sources().src);
    expect(report).toHaveBeenCalledWith(false);
  });
});
