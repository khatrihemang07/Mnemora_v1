<!-- Shortcuts: read-only reference. -->
<script lang="ts">
  import Kbd from '../../../lib/ui/Kbd.svelte';
  import SettingRow from '../../../lib/ui/SettingRow.svelte';
  import SettingSection from '../../../lib/ui/SettingSection.svelte';
  import { SHORTCUT_LIST } from '../../../shell/shortcuts';
</script>

<SettingSection title="Shortcuts">
  {#each SHORTCUT_LIST as s (s.label)}
    <SettingRow label={s.label}><Kbd keys={s.keys} /></SettingRow>
  {/each}
</SettingSection>
