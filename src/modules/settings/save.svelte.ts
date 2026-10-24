import { commands } from '../../lib/ipc/bindings';
import { unwrap } from '../../lib/ipc/unwrap';
import { settingsDraft } from '../../lib/settings/store.svelte';

/** Saves the draft (N9-B). A failure keeps the edits so the user can retry. */
class SaveState {
  saving = $state(false);
  error = $state<unknown>(null);

  async run(): Promise<boolean> {
    const changes = settingsDraft.changes();
    if (changes.length === 0) return true;
    this.saving = true;
    this.error = null;
    try {
      await unwrap(commands.settingsSave(changes));
      settingsDraft.commit();
      return true;
    } catch (e) {
      this.error = e;
      return false;
    } finally {
      this.saving = false;
    }
  }

  discard() {
    settingsDraft.discard();
    this.error = null;
  }
}

export const save = new SaveState();
