/**
 * Sync for the browser mock. Mirrors crates/sync (device name rules, the Device settings) and
 * src-tauri/src/sync.rs; fixtures/sync-rules.json keeps the two in step (ADR 0009).
 */
import type {
  AppError,
  DeviceSettings,
  DevicesView,
  PairingView,
  PeerState,
  SyncOption,
  SyncState,
} from './bindings';
import { createLiveMock, type PairedBase } from './mock-sync-live';
import { createPairingMock, defaultPairingFlags, type PairingFlags } from './mock-sync-pairing';

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
];

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
) {
  const peers: PairedBase[] = FAKE_PEERS.slice(0, pairedDevices).map((p) => ({ ...p }));
  const sync = () => {
    device.sync = {
      enabled: false,
      name: null,
      automatic: true,
      mobile_data: false,
      visible: true,
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
      paired_devices: peers.length,
    };
  };

  const pairing = createPairingMock({
    peers,
    emit,
    flags,
    myName: () => state().name,
    enabled: () => sync().enabled,
    visible: () => sync().visible,
  });

  const live = createLiveMock(peers, emitPeers, () => sync().enabled);
  // Console: `__syncMock.setPeer('syncing')` (see mock-sync-live.ts).
  flags.setPeer = (state: PeerState, name?: string) => live.setPeer(state, name);

  const devices = (): DevicesView => ({
    this_device: {
      name: state().name,
      platform: 'desktop',
      port: 47311,
      addresses: ['192.168.1.20', '100.101.4.7'],
    },
    peers: live.rows(),
  });

  return {
    handles: (cmd: string) => COMMANDS.includes(cmd) || pairing.commands.includes(cmd),
    handle(cmd: string, a: Record<string, unknown>): unknown {
      if (pairing.commands.includes(cmd)) return pairing.handle(cmd, a);
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
        case 'sync_rename':
          s.name = validateName(String(a.name));
          return null;
        case 'sync_set_option': {
          const option = a.option as SyncOption;
          s[option] = a.enabled === true;
          return null;
        }
        case 'sync_leave':
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
        default:
          return undefined;
      }
    },
  };
}
