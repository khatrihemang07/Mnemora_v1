<!-- Modules: the Modules that are on the way. -->
<script lang="ts">
  import BookOpen from '~icons/lucide/notebook-pen';
  import SquareCheck from '~icons/lucide/square-check-big';
  import Sparkles from '~icons/lucide/sparkles';
  import SettingRow from '../../../lib/ui/SettingRow.svelte';
  import SettingSection from '../../../lib/ui/SettingSection.svelte';

  const upcoming = [
    { name: 'Journal', icon: BookOpen, hue: '--hue-journal' },
    { name: 'Todo', icon: SquareCheck, hue: '--hue-todo' },
    { name: 'AI', icon: Sparkles, hue: '--hue-ai' },
  ];
</script>

<SettingSection title="Modules" description="Turn Modules on and off here once they arrive.">
  {#each upcoming as m (m.name)}
    <SettingRow label={m.name}>
      <span class="flex items-center gap-2 text-muted">
        <span style:color="var({m.hue})"><m.icon class="size-4" /></span>
        Coming soon
      </span>
    </SettingRow>
  {/each}
</SettingSection>
