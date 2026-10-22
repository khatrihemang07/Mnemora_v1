import { createQuery } from '@tanstack/svelte-query';
import { commands } from '../../lib/ipc/bindings';
import { unwrap } from '../../lib/ipc/unwrap';
import type { DeviceSwitch, ModuleContext } from '../../lib/router';
import { deviceQuery } from '../../lib/settings/queries';
import { notifyError } from '../../lib/ui/toast';
import { refreshSync } from './queries';

/** Whether the "Turn off Sync?" Dialog is open. It is mounted by the Module's overlay while Sync is on. */
class TurnOffDialog {
  open = $state(false);
}
export const turnOffDialog = new TurnOffDialog();

async function setEnabled(on: boolean): Promise<boolean> {
  try {
    await unwrap(commands.syncSetEnabled(on));
  } catch (e) {
    notifyError(e instanceof Error ? e.message : 'Could not change Sync');
    return false;
  } finally {
    // Wait for the fresh values: the shell routes by them, so `/sync` must be enabled before we go there.
    await refreshSync();
  }
  return true;
}

/** Confirms the Dialog: this device stops syncing; its data and paired devices are kept. */
export async function confirmTurnOff() {
  turnOffDialog.open = false;
  await setEnabled(false);
}

/**
 * Sync's on/off, a Device setting read from the device file (never synced). Turning on opens the
 * Module (its welcome page, until the first pairing); turning off asks first.
 */
export function useSyncSwitch(ctx: ModuleContext): DeviceSwitch {
  const device = createQuery(() => deviceQuery);
  return {
    get on() {
      return device.data?.sync?.enabled === true;
    },
    get ready() {
      return device.isSuccess;
    },
    set(on) {
      if (!on) {
        turnOffDialog.open = true;
        return;
      }
      void setEnabled(true).then((ok) => ok && ctx.navigate('/sync'));
    },
  };
}
