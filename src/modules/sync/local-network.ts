import { commands, type ThisDevice } from '../../lib/ipc/bindings';
import { unwrap } from '../../lib/ipc/unwrap';
import type { AndroidBridge } from '../../lib/theme/system-bars';

/**
 * The system keeping this device off the local network (D12 to D15): macOS 15+ Local Network
 * privacy, Android 17's local network permission. Rust decides "blocked" (`local_network_blocked`);
 * this file words it and acts on the buttons.
 */

/** The Welcome page's Mac-only line (D12). */
export const WELCOME_LINE =
  'Mnemora will ask to find devices on your local network. Allow it so your devices can find each other.';

export interface LocalNetworkBannerText {
  title: string;
  body: string;
  button: string;
}

/** The banner on Devices and in the Pair sheet, or `null` while nothing is blocked. */
export function localNetworkBanner(
  device: Pick<ThisDevice, 'platform' | 'local_network_blocked'> | undefined,
): LocalNetworkBannerText | null {
  if (!device?.local_network_blocked) return null;
  return device.platform === 'phone'
    ? {
        title: 'This phone can’t reach devices on your network.',
        body: 'Allow Mnemora to find devices on your local network in the app’s settings.',
        button: 'Open Settings',
      }
    : {
        title: 'This Mac can’t reach devices on your network.',
        body: 'Allow Mnemora under System Settings › Privacy & Security › Local Network.',
        button: 'Open Settings',
      };
}

/** "Or on your phone: Pair by address… 192.168.1.20": what the Mac's Pair sheet adds under the code. */
export function pairByAddressHint(
  device: Pick<ThisDevice, 'platform' | 'addresses'> | undefined,
): { before: string; address: string } | null {
  const address = device?.addresses[0];
  if (!address || device.platform !== 'desktop') return null;
  return { before: 'Or on your phone: Pair by address…', address };
}

/** What the banner's button does: the phone asks (or opens the app's settings), the Mac opens System Settings. */
export async function openLocalNetworkSettings(
  platform: ThisDevice['platform'],
  bridge: AndroidBridge | undefined = window.MnemoraAndroid,
): Promise<void> {
  if (platform === 'phone') {
    if (typeof bridge?.requestLocalNetwork === 'function') bridge.requestLocalNetwork(true);
    else bridge?.openAppSettings?.();
    return;
  }
  await unwrap(commands.syncOpenLocalNetworkSettings());
}

/**
 * Sync was just turned on: on Android 17+ the system is asked for the local network permission now
 * (never while Sync is off). No-op elsewhere.
 */
export function askLocalNetworkOnEnable(bridge: AndroidBridge | undefined = window.MnemoraAndroid) {
  if (typeof bridge?.requestLocalNetwork === 'function') bridge.requestLocalNetwork(false);
}

/** The parts of `window` and `document` that say the permission may have changed. */
export interface PermissionSources {
  win: {
    addEventListener(type: 'mnemora-local-network', listener: () => void): void;
    removeEventListener(type: 'mnemora-local-network', listener: () => void): void;
  };
  doc: {
    hidden: boolean;
    addEventListener(type: 'visibilitychange', listener: () => void): void;
    removeEventListener(type: 'visibilitychange', listener: () => void): void;
  };
}

/**
 * While Sync is on (the overlay mounts this), tells Rust whether Android's local network permission
 * is denied: now, when the app comes back to the front and when the dialog is answered. Returns what
 * stops it. No-op without the bridge (desktop, browser) and where the permission does not exist.
 */
export function reportLocalNetworkPermission(
  bridge: AndroidBridge | undefined,
  report: (denied: boolean) => void,
  { win, doc }: PermissionSources = { win: window, doc: document },
): () => void {
  const read = bridge?.localNetworkState;
  if (typeof read !== 'function') return () => {};
  const tell = () => {
    if (doc.hidden) return;
    const state = read.call(bridge);
    report(state === 'denied');
  };
  tell();
  doc.addEventListener('visibilitychange', tell);
  win.addEventListener('mnemora-local-network', tell);
  return () => {
    doc.removeEventListener('visibilitychange', tell);
    win.removeEventListener('mnemora-local-network', tell);
    // Sync is off or the page is leaving: nothing is blocked any more.
    report(false);
  };
}
