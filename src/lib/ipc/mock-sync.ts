/**
 * Sync for the browser mock. Mirrors crates/sync (device name rules, the Device settings) and
 * src-tauri/src/sync.rs; fixtures/sync-rules.json keeps the two in step (ADR 0009).
 */
import type { AppError, DeviceSettings, SyncOption, SyncState } from './bindings';

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

const COMMANDS = ['sync_get', 'sync_set_enabled', 'sync_rename', 'sync_set_option', 'sync_leave'];

/** `device` is the mock's device file; Sync's choices live in its `sync` table, as in Rust. */
export function createSyncMock(device: DeviceSettings, pairedDevices = 0) {
  let paired = pairedDevices;
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
      paired_devices: paired,
    };
  };

  return {
    handles: (cmd: string) => COMMANDS.includes(cmd),
    handle(cmd: string, a: Record<string, unknown>): unknown {
      const s = sync();
      switch (cmd) {
        case 'sync_get':
          return state();
        case 'sync_set_enabled':
          s.enabled = a.enabled === true;
          // The first time it is turned on, the device is named after the computer.
          if (s.enabled && s.name === null) s.name = COMPUTER_NAME;
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
          paired = 0;
          return null;
        default:
          return undefined;
      }
    },
  };
}
