<!-- TooltipProvider: mount once near the root; sets the shared 600ms delay. -->
<script lang="ts">
  import { Tooltip } from 'bits-ui';
  import type { Snippet } from 'svelte';
  let { children }: { children: Snippet } = $props();
</script>

<Tooltip.Provider delayDuration={600}>{@render children()}</Tooltip.Provider>
