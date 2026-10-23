<!--
  SyncSettings: Sync's section in Settings. Every row is a Device setting (this device only, never
  synced), so each saves at once through its own command, not through the shared draft and SaveBar.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { commands, type SyncOption } from '../../lib/ipc/bindings';
  import { unwrap } from '../../lib/ipc/unwrap';
  import Button from '../../lib/ui/Button.svelte';
  import Dialog from '../../lib/ui/Dialog.svelte';
  import SettingRow from '../../lib/ui/SettingRow.svelte';
  import SettingSection from '../../lib/ui/SettingSection.svelte';
  import SegmentedControl from '../../lib/ui/SegmentedControl.svelte';
  import Switch from '../../lib/ui/Switch.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import { viewport } from '../../lib/viewport.svelte';
  import DeviceNameField from './DeviceNameField.svelte';
  import { refreshSync, syncQuery } from './queries';
  import { turnOffDialog } from './switch.svelte';

  const sync = createQuery(() => syncQuery);
  const paired = $derived(sync.data?.paired_devices ?? 0);
  let leaving = $state(false);
  // Bumped after asking to turn off: the switch snaps back until the Dialog is confirmed.
  let flip = $state(0);

  async function run(call: ReturnType<typeof commands.syncLeave>) {
    try {
      await unwrap(call);
    } catch (e) {
      notifyError(e instanceof Error ? e.message : 'Could not save');
    }
    await refreshSync();
  }

  const setOption = (option: SyncOption, on: boolean) =>
    void run(commands.syncSetOption(option, on));

  async function leave() {
    leaving = false;
    await run(commands.syncLeave());
  }
</script>

<SettingSection title="Sync">
  <SettingRow label="Sync this device" hint="Only this device. Your other devices keep their own choice.">
    {#key flip}
      <Switch
        label="Sync this device"
        checked={sync.data?.enabled ?? true}
        onchange={(on) => {
          if (!on) turnOffDialog.open = true;
          flip++;
        }}
      />
    {/key}
  </SettingRow>
  <SettingRow label="This device’s name" hint="Shown on your other devices.">
    <DeviceNameField class="min-[720px]:w-64" />
  </SettingRow>
  <SettingRow
    label="Sync automatically"
    hint="When devices are around and after each change. Off: only Sync now."
  >
    <Switch
      label="Sync automatically"
      checked={sync.data?.automatic ?? true}
      onchange={(on) => setOption('automatic', on)}
    />
  </SettingRow>
  {#if viewport.isPhone}
    <SettingRow
      label="Allow over mobile data"
      hint="Reaches devices over Tailscale away from Wi-Fi."
    >
      <Switch
        label="Allow over mobile data"
        checked={sync.data?.mobile_data ?? false}
        onchange={(on) => setOption('mobile_data', on)}
      />
    </SettingRow>
  {/if}
  <SettingRow label="Visible on this network" hint="Lets your paired devices find this one nearby.">
    <Switch
      label="Visible on this network"
      checked={sync.data?.visible ?? true}
      onchange={(on) => setOption('visible', on)}
    />
  </SettingRow>
  <SettingRow label="New conflicts" hint="On this device only.">
    <SegmentedControl
      label="New conflicts"
      value={sync.data?.toast_conflicts ? 'toast' : 'quietly'}
      options={[
        { value: 'quietly', label: 'Quietly' },
        { value: 'toast', label: 'Show a toast' },
      ]}
      onchange={(v) => setOption('toast_conflicts', v === 'toast')}
    />
  </SettingRow>
</SettingSection>

<SettingSection title="Danger zone">
  <SettingRow
    label="Leave and forget all devices"
    hint={paired === 0
      ? 'No paired devices yet.'
      : 'This device stops syncing with them for good. Its data stays here.'}
  >
    <Button variant="danger" size="sm" disabled={paired === 0} onclick={() => (leaving = true)}>
      Leave…
    </Button>
  </SettingRow>
</SettingSection>

<Dialog
  open={leaving}
  title="Leave and forget all devices?"
  description="This device forgets all {paired} paired {paired === 1 ? 'device' : 'devices'} and stops syncing with them. Its data stays here. To sync again you pair again."
  onclose={() => (leaving = false)}
>
  {#snippet actions()}
    <Button onclick={() => (leaving = false)}>Cancel</Button>
    <Button variant="danger" onclick={() => void leave()}>Leave</Button>
  {/snippet}
</Dialog>
