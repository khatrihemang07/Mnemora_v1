<!-- ModuleOverlays: the always-on components of the enabled Modules (Todo's floating Quick add). -->
<script lang="ts">
  import type { ModuleManifest } from '../lib/router';
  import { overlayModules } from './enabled-modules';

  let { enabled }: { enabled: ModuleManifest[] } = $props();
  const mounted = $derived(overlayModules(enabled));
</script>

{#each mounted as m (m.id)}
  {#await m.overlay?.() then loaded}
    {#if loaded}<loaded.default />{/if}
  {/await}
{/each}
