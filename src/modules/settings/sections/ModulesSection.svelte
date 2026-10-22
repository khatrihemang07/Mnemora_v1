<!-- Modules: switch Modules on or off. Off hides them everywhere and keeps their data. -->
<script lang="ts">
  import SettingSection from '../../../lib/ui/SettingSection.svelte';
  import { modules } from '../../../shell/registry';
  import ModuleSwitch from './ModuleSwitch.svelte';

  const rows = $derived([...modules].sort((a, b) => a.nav.order - b.nav.order));
</script>

<SettingSection title="Modules" description="Turn a Module off to hide it everywhere. Its data is kept.">
  {#each rows as m (m.id)}
    <ModuleSwitch {m} />
  {/each}
</SettingSection>
