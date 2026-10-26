<!--
  A list of actions for this Module: the shared Menu on a desktop, a Sheet of the same rows on a
  phone. The caller owns `open` and says what it hangs off (`anchor`, desktop only).
-->
<script lang="ts">
  import type { MenuItem } from '../../lib/ui/Menu.svelte';
  import Menu from '../../lib/ui/Menu.svelte';
  import Sheet from '../../lib/ui/Sheet.svelte';
  import { viewport } from '../../lib/viewport.svelte';

  interface Props {
    open: boolean;
    anchor: HTMLElement | null;
    items: MenuItem[];
    /** Names the menu for screen readers, and titles the Sheet on a phone. */
    label: string;
    /** Where the desktop menu opens relative to its anchor. */
    side?: 'top' | 'bottom';
  }

  let { open = $bindable(), anchor, items, label, side = 'bottom' }: Props = $props();

  function pick(item: MenuItem) {
    open = false;
    item.onselect();
  }
</script>

{#if viewport.isPhone}
  <Sheet {open} title={label} onclose={() => (open = false)}>
    <div class="flex flex-col gap-0.5 pb-2">
      {#each items as item (item.label)}
        {#if item.separatorBefore}
          <div class="my-1 h-px bg-line"></div>
        {/if}
        <button
          type="button"
          disabled={item.disabled}
          class="flex min-h-11 w-full items-center gap-3 rounded-sm px-3 text-left text-base transition-colors duration-(--dur) ease-out hover:bg-hover disabled:pointer-events-none disabled:opacity-50 {item.danger ? 'text-danger' : 'text-ink'}"
          onclick={() => pick(item)}
        >
          {#if item.icon}<item.icon class="size-5 shrink-0 {item.danger ? '' : 'text-muted'}" />{/if}
          <span class="min-w-0 flex-1 truncate">{item.label}</span>
        </button>
      {/each}
    </div>
  </Sheet>
{:else}
  <Menu bind:open {anchor} {items} {label} {side} />
{/if}
