/**
 * Sync for the browser mock. Mirrors crates/sync (device name rules, the Device settings) and
 * src-tauri/src/sync.rs; fixtures/sync-rules.json keeps the two in step (ADR 0009).
 */

import { parseAddress, routeOfAddress } from '../../modules/sync/address';
import type {
  AppError,
  Attention,
  DeviceSettings,
  DevicesView,
  PairingView,
  PeerRow,
  PeerState,
  Platform,
  SyncOption,
  SyncState,
} from './bindings';
import { type Event, lineNow } from './mock-sync-activity';
import { createDeviceMock } from './mock-sync-devices';
import { createLiveMock, type PairedBase } from './mock-sync-live';
import {
  createPairingMock,
  defaultPairingFlags,
  type PairingFlags,
  uniqueName,
} from './mock-sync-pairing';

const MAX_NAME_CHARS = 40;
const COMPUTER_NAME = 'Alex’s MacBook Pro';

const err = (message: string): AppError => ({ code: 'validation', message, details: null });

/** Mirrors `validate_name` in crates/sync/src/name.rs. */
export function validateName(raw: string): string {
  const name = raw.trim();
  const length = [...name].length;
  if (length === 0) throw err('Give this device a name');
  if (length > MAX_NAME_CHARS) throw err('A device name is at most 40 characters');
  return name;
}

const COMMANDS = [
  'sync_get',
  'sync_set_enabled',
  'sync_rename',
  'sync_set_option',
  'sync_leave',
  'sync_devices',
  'sync_now',
  'sync_resume',
  'sync_attention',
  'sync_open_local_network_settings',
  'sync_local_network_os',
];

const DAY_MS = 86_400_000;

/** Mirrors `attention_of` in crates/sync/src/attention.rs: what the Home card, badge and hint read. */
export function attentionOf(
  conflicts: number,
  peers: PeerRow[],
  now: number,
  localNetworkBlocked = false,
): Attention {
  const paused = peers.filter((p) => p.state === 'paused').map((p) => p.name);
  const unseen = peers
    .filter((p) => p.state === 'not_nearby')
    .map((p) => ({
      name: p.name,
      days: Math.floor((now - (p.last_seen_at ?? p.paired_at)) / DAY_MS),
    }))
    .filter((u) => u.days >= 14);
  const last = peers
    .filter((p) => p.last_synced_at !== null)
    .sort((a, b) => (b.last_synced_at ?? 0) - (a.last_synced_at ?? 0))[0];
  return {
    conflicts,
    paused,
    unseen,
    last_synced: last ? { device: last.name, at: last.last_synced_at ?? 0 } : null,
    local_network_blocked: localNetworkBlocked,
    needed: conflicts > 0 || paused.length > 0 || localNetworkBlocked || unseen.length > 0,
  };
}

/** Two devices already paired, for `?paired=2`. */
const FAKE_PEERS: PairedBase[] = [
  {
    id: 'mock-pixel',
    name: 'Pixel 8 (simulated)',
    platform: 'phone',
    paired_at: 1_790_000_000_000,
  },
  {
    id: 'mock-air',
    name: 'MacBook Air (simulated)',
    platform: 'desktop',
    paired_at: 1_780_000_000_000,
  },
];

/** `device` is the mock's device file; Sync's choices live in its `sync` table, as in Rust. */
export function createSyncMock(
  device: DeviceSettings,
  pairedDevices = 0,
  emit: (view: PairingView) => void = () => {},
  flags: PairingFlags = defaultPairingFlags(),
  emitPeers: () => void = () => {},
  openConflicts: () => number = () => 0,
  /** Writes a line to Activity (Leave, Forget and Rename each log one, as in Rust). */
  log: (line: Event) => void = () => {},
) {
  const peers: PairedBase[] = FAKE_PEERS.slice(0, pairedDevices).map((p) => ({ ...p }));
  const sync = () => {
    device.sync = {
      enabled: false,
      name: null,
      automatic: true,
      mobile_data: false,
      visible: true,
      toast_conflicts: false,
      ...device.sync,
    };
    return device.sync as Required<NonNullable<DeviceSettings['sync']>>;
  };

  const state = (): SyncState => {
    const s = sync();
    return {
      enabled: s.enabled,
      name: s.name ?? COMPUTER_NAME,
      automatic: s.automatic,
      mobile_data: s.mobile_data,
      visible: s.visible,
      toast_conflicts: s.toast_conflicts,
      paired_devices: peers.length,
    };
  };

  /** Mirrors `paused_on_mobile_data` in src-tauri/src/sync_power.rs. */
  const pausedOnMobile = () => flags.onMobile === true && sync().enabled && !sync().mobile_data;
  // The system keeping this device off the local network (D13; `flags.localNetwork`, `?localnet=`).
  const net = { blocked: false, platform: 'desktop' as Platform, loggedAt: 0 };
  const blockedNow = () => net.blocked && sync().enabled;
  flags.localNetwork = (state, platform = 'desktop') => {
    const was = blockedNow();
    net.blocked = state === 'blocked';
    net.platform = platform;
    // One Activity line when it starts, not again within ten minutes (Rust: log_local_network_blocked).
    if (!was && blockedNow() && Date.now() - net.loggedAt > 600_000) {
      net.loggedAt = Date.now();
      log(lineNow('blocked', Date.now()));
    }
    emitPeers();
  };
  /**
   * Mirrors `with_mobile_pause` then `with_local_network_block`: what cannot be reached says why
   * instead of "Not nearby".
   */
  const rows = (): PeerRow[] =>
    live.rows().map((r) => {
      if (r.state !== 'not_nearby') return r;
      if (pausedOnMobile()) return { ...r, state: 'paused_on_mobile_data' };
      return blockedNow() ? { ...r, state: 'blocked_by_local_network' } : r;
    });

  const typedAddresses = new Map<string, string>();
  const pairing = createPairingMock({
    rememberAddress: (id, stored) => {
      typedAddresses.set(id, stored);
      // Reached by that address, so the route is the one the address leads to.
      live.setRoute(id, routeOfAddress(stored));
    },
    peers,
    emit,
    flags,
    myName: () => state().name,
    enabled: () => sync().enabled,
    pausedOnMobile: () => pausedOnMobile(),
    visible: () => sync().visible,
  });

  const live = createLiveMock(peers, emitPeers, () => sync().enabled);
  // Console: `__syncMock.setPeer('syncing')` (see mock-sync-live.ts).
  flags.setPeer = (state: PeerState, name?: string, daysAway?: number, why?: string) =>
    live.setPeer(state, name, daysAway, why);

  // Console: `__syncMock.memberTypedAddress('100.64.0.7')`. The Address of a device is one value
  // for the whole group (newest edit wins), so what another member typed shows here too.
  flags.memberTypedAddress = (address: string, id?: string) => {
    const peer = peers.find((p) => p.id === id) ?? peers[0];
    if (!peer) return;
    const parsed = parseAddress(address);
    if (address.trim() === '') typedAddresses.delete(peer.id);
    else if (parsed.ok) typedAddresses.set(peer.id, parsed.stored);
    emitPeers();
  };

  const deviceSheet = createDeviceMock({
    typed: typedAddresses,
    peers,
    rows,
    myName: () => state().name,
    emitPeers,
    syncOne: (id) => live.syncOne(id),
    forget: (id) => live.forget(id),
    log,
    forced: () => flags.check,
  });

  const devices = (): DevicesView => ({
    this_device: {
      name: state().name,
      platform: net.platform,
      port: 47311,
      addresses: ['192.168.1.20', '100.101.4.7'],
      routes: ['wifi', 'tailscale'],
      paused_on_mobile_data: pausedOnMobile(),
      local_network_blocked: blockedNow(),
    },
    peers: rows(),
  });

  return {
    pairedCount: () => peers.length,
    handles: (cmd: string) =>
      COMMANDS.includes(cmd) ||
      pairing.commands.includes(cmd) ||
      deviceSheet.commands.includes(cmd),
    handle(cmd: string, a: Record<string, unknown>): unknown {
      if (pairing.commands.includes(cmd)) return pairing.handle(cmd, a);
      if (deviceSheet.commands.includes(cmd)) return deviceSheet.handle(cmd, a);
      const s = sync();
      switch (cmd) {
        case 'sync_get':
          return state();
        case 'sync_set_enabled':
          s.enabled = a.enabled === true;
          // The first time it is turned on, the device is named after the computer.
          if (s.enabled && s.name === null) s.name = COMPUTER_NAME;
          // Off stops everything, pairing included.
          if (!s.enabled) {
            pairing.handle('sync_pair_cancel', {});
            live.reset();
          }
          return null;
        case 'sync_rename': {
          const name = validateName(String(a.name));
          if (
            uniqueName(
              name,
              peers.map((p) => p.name),
            ) !== name
          ) {
            throw err('Another of your devices already has that name');
          }
          const old = s.name ?? COMPUTER_NAME;
          s.name = name;
          // Like rename.rs: a rename is logged when there is a group to tell (R1, story 85).
          if (old !== name && peers.length > 0)
            log(lineNow('renamed', Date.now(), { extra: { old, new: name } }));
          return null;
        }
        case 'sync_set_option': {
          const option = a.option as SyncOption;
          s[option] = a.enabled === true;
          return null;
        }
        case 'sync_leave':
          if (peers.length > 0) log(lineNow('left', Date.now()));
          peers.length = 0;
          live.reset();
          return null;
        case 'sync_now':
          live.syncNow();
          return null;
        case 'sync_resume':
          return null;
        case 'sync_devices':
          return devices();
        case 'sync_attention':
          return attentionOf(openConflicts(), live.rows(), Date.now(), blockedNow());
        case 'sync_open_local_network_settings':
          // The real command opens System Settings; nothing to open in a browser.
          console.info('[mock] Open Settings › Privacy & Security › Local Network');
          return null;
        case 'sync_local_network_os':
          // Android only: the page tells Rust what the system said. Steer the mock with `flags.localNetwork`.
          return null;
        default:
          return undefined;
      }
    },
  };
}
