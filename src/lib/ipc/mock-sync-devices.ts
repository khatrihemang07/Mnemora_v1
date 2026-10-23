/**
 * The device sheet for the browser mock: what is known about a paired device, Rename, the typed
 * Address, Check and Forget. Mirrors crates/sync/src/detail.rs, check.rs, rename.rs and revoke.rs
 * and the commands in src-tauri/src/sync.rs.
 *
 * Check has no network to dial, so the typed text decides what it finds:
 *   anything valid          Reached ‹name› in 84 ms        …closed…      nothing answered
 *   …timeout…               timed out                       …nowhere…     the name does not resolve
 *   …wrong…                 another device answered         …unpaired…    not a paired device
 * In the console, `__syncMock.check = 'unreachable' | 'timed_out' | …` forces one answer for every
 * Check (`'reached'` to go back to the text deciding).
 */
import { parseAddress } from '../../modules/sync/address';
import type {
  AppError,
  CheckKind,
  CheckResult,
  DeviceDetail,
  KnownAddress,
  PeerRow,
} from './bindings';
import type { PairedBase } from './mock-sync-live';
import { uniqueName } from './mock-sync-pairing';

const MIN = 60_000;
const DAY = 24 * 60 * MIN;
const MAX_NAME_CHARS = 40;

export const DEVICE_COMMANDS = [
  'sync_device',
  'sync_device_rename',
  'sync_device_address',
  'sync_device_check',
  'sync_device_sync_now',
  'sync_device_forget',
];

const error = (code: AppError['code'], message: string): AppError => ({
  code,
  message,
  details: null,
});

/** What a Check says for each outcome (the words of crates/sync/src/check.rs). */
export function checkResult(kind: CheckKind, name: string, ms = 84): CheckResult {
  switch (kind) {
    case 'reached':
      return { reached: true, kind, message: `Reached ${name} in ${ms} ms`, ms };
    case 'not_found':
      return {
        reached: false,
        kind,
        message: 'Could not find that name. Check the spelling and that Tailscale is connected.',
        ms: null,
      };
    case 'timed_out':
      return {
        reached: false,
        kind,
        message: `Timed out. ${name} may be asleep, off or on another network.`,
        ms: null,
      };
    case 'wrong_device':
      return { reached: false, kind, message: `That is Bob, not ${name}.`, ms: null };
    case 'not_paired':
      return {
        reached: false,
        kind,
        message: 'That is not a device paired with this one, or it has forgotten this device.',
        ms: null,
      };
    case 'other':
      return {
        reached: false,
        kind,
        message: 'Something answered, but it is not Mnemora.',
        ms: null,
      };
    default:
      return {
        reached: false,
        kind: 'unreachable',
        message: 'Nothing answered there. Is Mnemora open with Sync on?',
        ms: null,
      };
  }
}

/** The kind a typed text stands for in the mock (see the header). */
function kindFor(text: string): CheckKind {
  const t = text.toLowerCase();
  if (t.includes('timeout')) return 'timed_out';
  if (t.includes('nowhere')) return 'not_found';
  if (t.includes('wrong')) return 'wrong_device';
  if (t.includes('unpaired')) return 'not_paired';
  if (t.includes('closed')) return 'unreachable';
  return 'reached';
}

function knownAddresses(peer: PairedBase, now: number): KnownAddress[] {
  if (peer.id === 'mock-pixel') {
    return [
      { address: '192.168.1.31:47311', last_ok_at: now - 2 * MIN },
      { address: '100.88.12.4:47311', last_ok_at: now - 3 * DAY },
    ];
  }
  if (peer.id === 'mock-air') {
    return [{ address: '100.88.12.5:47311', last_ok_at: now - 3 * DAY }];
  }
  return [{ address: '192.168.1.40:47311', last_ok_at: now }];
}

export interface DeviceMockOptions {
  peers: PairedBase[];
  rows: () => PeerRow[];
  myName: () => string;
  emitPeers: () => void;
  syncOne: (id: string) => void;
  forget: (id: string) => void;
  /** Console override for every Check. */
  forced: () => CheckKind | undefined;
  /** Typed Addresses by device id; shared with pairing by address. */
  typed?: Map<string, string>;
}

export function createDeviceMock(o: DeviceMockOptions) {
  const typed = o.typed ?? new Map<string, string>();
  const find = (id: unknown): PairedBase => {
    const peer = o.peers.find((p) => p.id === id);
    if (!peer) throw error('not_found', 'That device is not paired');
    return peer;
  };

  function detail(peer: PairedBase): DeviceDetail {
    const row = o.rows().find((r) => r.id === peer.id);
    const now = Date.now();
    return {
      id: peer.id,
      name: peer.name,
      platform: peer.platform,
      state: row?.state ?? 'not_nearby',
      paired_at: peer.paired_at,
      paired_by: o.myName(),
      last_seen_at: row?.last_seen_at ?? null,
      last_synced_at: row?.last_synced_at ?? null,
      route: row?.route ?? null,
      addresses: knownAddresses(peer, now),
      typed_address: typed.get(peer.id) ?? null,
      app_version: '0.9.2',
      data_versions: [
        { module: 'core', version: 4 },
        { module: 'journal', version: 3 },
        { module: 'sync', version: 10 },
        { module: 'todo', version: 9 },
      ],
    };
  }

  return {
    commands: DEVICE_COMMANDS,
    handle(cmd: string, a: Record<string, unknown>): unknown {
      const peer = find(a.id);
      switch (cmd) {
        case 'sync_device':
          return detail(peer);
        case 'sync_device_rename': {
          const name = String(a.name).trim();
          const length = [...name].length;
          if (length === 0) throw error('validation', 'Give this device a name');
          if (length > MAX_NAME_CHARS)
            throw error('validation', 'A device name is at most 40 characters');
          const others = [o.myName(), ...o.peers.filter((p) => p !== peer).map((p) => p.name)];
          if (uniqueName(name, others) !== name) {
            throw error('validation', 'Another of your devices already has that name');
          }
          peer.name = name;
          o.emitPeers();
          return name;
        }
        case 'sync_device_address': {
          const text = String(a.address);
          if (text.trim() === '') {
            typed.delete(peer.id);
            return null;
          }
          const parsed = parseAddress(text);
          if (!parsed.ok) throw error('validation', parsed.message);
          typed.set(peer.id, parsed.stored);
          return parsed.stored;
        }
        case 'sync_device_check': {
          const text = String(a.address);
          const parsed = parseAddress(text);
          if (!parsed.ok)
            return { reached: false, kind: 'invalid', message: parsed.message, ms: null };
          return checkResult(o.forced() ?? kindFor(text), peer.name);
        }
        case 'sync_device_sync_now':
          o.syncOne(peer.id);
          return null;
        case 'sync_device_forget':
          typed.delete(peer.id);
          o.forget(peer.id);
          return null;
        default:
          return undefined;
      }
    },
  };
}
