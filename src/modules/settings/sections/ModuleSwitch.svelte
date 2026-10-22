<!--
  ModuleSwitch: one row of Settings ▸ Modules. A Module that is on by default is listed in
  `core.disabled_modules`, one that is off by default in `core.enabled_optional_modules` (both
  Preferences, so they wait for Save), and one whose switch is a Device setting (Sync) asks its own
  switch, which applies at once and may ask first.
-->
<script lang="ts">
  import type { ModuleManifest } from '../../../lib/router';
  import { readCore } from '../../../lib/settings/core';
  import { settingsDraft as d } from '../../../lib/settings/store.svelte';
  import SettingRow from '../../../lib/ui/SettingRow.svelte';
  import Switch from '../../../lib/ui/Switch.svelte';
  import { setModuleEnabled, setOptedIn } from '../../../shell/enabled-modules';
  import { moduleContext } from '../../../shell/module-context';

  let { m }: { m: ModuleManifest } = $props();

  // The manifest of a row never changes, so its switch is made once, as component setup.
  // svelte-ignore state_referenced_locally
  const device = m.deviceSwitch?.(moduleContext);
  // The draft, not the saved value: the switches show unsaved edits until Save.
  const core = $derived(readCore(d.merged()));
  const optIn = $derived(m.defaultEnabled === false);
  const checked = $derived(
    m.required
      ? true
      : device
        ? device.on
        : optIn
          ? core.enabledOptionalModules.includes(m.id)
          : !core.disabledModules.includes(m.id),
  );
  // Asking to turn a device switch off opens a Dialog, so the row must show it still on until then.
  let revision = $state(0);

  function change(on: boolean) {
    if (device) {
      device.set(on);
      revision++;
    } else if (optIn) {
      d.set('core.enabled_optional_modules', setOptedIn(core.enabledOptionalModules, m.id, on));
    } else {
      d.set('core.disabled_modules', setModuleEnabled(core.disabledModules, m.id, on));
    }
  }
</script>

<SettingRow label={m.name} hint={m.required ? 'Always on.' : m.description}>
  <span class="flex items-center gap-3">
    <span style:color="var({m.accentVar})"><m.icon class="size-4" /></span>
    {#key revision}
      <Switch label={`${m.name} Module`} {checked} disabled={m.required} onchange={change} />
    {/key}
  </span>
</SettingRow>
