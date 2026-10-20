<!--
  Tooltip: 600ms delay (N12-B / N17-A), shows the shortcut as Kbd (N16-A).
  The trigger snippet receives `props` to spread onto the element that should open it.
-->
<script lang="ts">
  import { Tooltip } from 'bits-ui';
  import type { Snippet } from 'svelte';
  import { viewport } from '../viewport.svelte';
  import Kbd from './Kbd.svelte';
  import { shownShortcut, tooltipAllowedNow, trackPointerType } from './tooltip-gate';

  interface Props {
    label: string;
    shortcut?: string;
    side?: 'top' | 'right' | 'bottom' | 'left';
    trigger: Snippet<[{ props: Record<string, unknown> }]>;
  }
  let { label, shortcut, side = 'top', trigger }: Props = $props();

  // A tap focuses the button, which would open the tooltip: ignore opens that follow touch.
  trackPointerType();
  let open = $state(false);
  const hint = $derived(shownShortcut(shortcut, viewport.isPhone));
</script>

<Tooltip.Root delayDuration={600} {open} onOpenChange={(o) => (open = o && tooltipAllowedNow())}>
  <Tooltip.Trigger>
    {#snippet child({ props })}
      {@render trigger({ props })}
    {/snippet}
  </Tooltip.Trigger>
  <Tooltip.Portal>
    <Tooltip.Content
      {side}
      sideOffset={6}
      class="z-50 flex items-center gap-2 rounded-sm bg-pop px-2 py-1 text-xs text-ink shadow-pop"
    >
      {label}
      {#if hint}<Kbd keys={hint} />{/if}
    </Tooltip.Content>
  </Tooltip.Portal>
</Tooltip.Root>
