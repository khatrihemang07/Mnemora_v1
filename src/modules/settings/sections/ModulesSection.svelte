<!-- Modules: switch Modules on or off. Off hides them everywhere and keeps their data. -->
<script lang="ts">
  import { readCore } from '../../../lib/settings/core';
  import { settingsDraft as d } from '../../../lib/settings/store.svelte';
  import SettingRow from '../../../lib/ui/SettingRow.svelte';
  import SettingSection from '../../../lib/ui/SettingSection.svelte';
  import Switch from '../../../lib/ui/Switch.svelte';
  import { setModuleEnabled } from '../../../shell/enabled-modules';
  import { modules } from '../../../shell/registry';

  const KEY = 'core.disabled_modules';
  // The draft, not the saved value: the switches show unsaved edits until Save.
  const disabled = $derived(readCore(d.merged()).disabledModules);
  const rows = $derived([...modules].sort((a, b) => a.nav.order - b.nav.order));
</script>

<SettingSection title="Modules" description="Turn a Module off to hide it everywhere. Its data is kept.">
  {#each rows as m (m.id)}
    <SettingRow label={m.name} hint={m.required ? 'Always on.' : undefined}>
      <span class="flex items-center gap-3">
        <span style:color="var({m.accentVar})"><m.icon class="size-4" /></span>
        <Switch
          label={`${m.name} Module`}
          checked={m.required ? true : !disabled.includes(m.id)}
          disabled={m.required}
          onchange={(on) => d.set(KEY, setModuleEnabled(disabled, m.id, on))}
        />
      </span>
    </SettingRow>
  {/each}
</SettingSection>
