<!-- Data & storage: Environment, data folder, manual backup. Nothing here is a draft. -->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { commands } from '../../../lib/ipc/bindings';
  import { unwrap } from '../../../lib/ipc/unwrap';
  import { appInfoQuery } from '../../../lib/settings/queries';
  import Button from '../../../lib/ui/Button.svelte';
  import EnvBadge from '../../../lib/ui/EnvBadge.svelte';
  import SettingRow from '../../../lib/ui/SettingRow.svelte';
  import SettingSection from '../../../lib/ui/SettingSection.svelte';
  import { MINOR_MS, notify, notifyError } from '../../../lib/ui/toast';

  const info = createQuery(() => appInfoQuery);
  let backingUp = $state(false);

  async function backup() {
    backingUp = true;
    try {
      notify(`Backup saved to ${await unwrap(commands.backupNow())}`);
    } catch (e) {
      notifyError(e instanceof Error ? e.message : 'Backup failed');
    } finally {
      backingUp = false;
    }
  }

  async function openFolder(path: string) {
    try {
      const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
      await revealItemInDir(path);
    } catch {
      notifyError('Could not open the folder', MINOR_MS);
    }
  }
</script>

<SettingSection title="Data & storage">
  <SettingRow label="Environment" hint="Which data set the app is using.">
    <span class="flex items-center gap-2 capitalize">
      {info.data?.environment ?? '…'}
      {#if info.data}<EnvBadge env={info.data.environment} />{/if}
    </span>
  </SettingRow>
  <SettingRow label="Data folder" hint={info.data?.data_dir}>
    <Button size="sm" disabled={!info.data} onclick={() => info.data && openFolder(info.data.data_dir)}>
      Open
    </Button>
  </SettingRow>
  <SettingRow label="Back up now" hint="Auto-backup before updates · keeps last 5.">
    <Button size="sm" disabled={backingUp} onclick={backup}>
      {backingUp ? 'Backing up…' : 'Back up now'}
    </Button>
  </SettingRow>
</SettingSection>
