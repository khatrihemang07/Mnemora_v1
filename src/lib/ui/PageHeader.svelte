<!--
  PageHeader: the page title in the display font, plus an optional actions slot.
  Desktop: hands the title to the shell's top bar (next to the sidebar toggle), renders nothing here.
  Phone: renders a large title at the top of the content.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import EnvBadge from './EnvBadge.svelte';
  import { viewport } from '../viewport.svelte';
  import { pageHeader } from './page-header.svelte';

  let { title, actions }: { title: string; actions?: Snippet } = $props();

  $effect(() => {
    if (viewport.isPhone) return;
    return pageHeader.set({ title, actions });
  });
</script>

{#if viewport.isPhone}
  <div class="mb-3 flex items-center justify-between gap-3">
    <div class="flex min-w-0 items-center gap-2">
      <h1 class="font-display text-2xl" style="font-weight: var(--display-weight)">{title}</h1>
      {#if pageHeader.env}<EnvBadge env={pageHeader.env} />{/if}
    </div>
    <!-- One group, so several actions sit together on the right rather than spread out. -->
    {#if actions}<div class="flex items-center gap-1">{@render actions()}</div>{/if}
  </div>
{/if}
