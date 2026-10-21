<!--
  PriorityMenu: the four Priorities next to `anchor`: P1 red, P2 amber, P3 violet flags and "No
  priority". A menu on desktop (the key 1 to 4 shows beside each), a bottom Sheet on a phone, where
  the row's ⋯ button the menu would hang off is not shown. `current` is ticked; `onpick` gets 1 to 4.
-->
<script lang="ts">
  import Check from '~icons/lucide/check';
  import Flag from '~icons/lucide/flag';
  import Menu, { type MenuAnchor, type MenuItem } from '../../lib/ui/Menu.svelte';
  import Sheet from '../../lib/ui/Sheet.svelte';
  import { viewport } from '../../lib/viewport.svelte';
  import { PRIORITY_NAMES } from './tasks';

  interface Props {
    open: boolean;
    anchor: MenuAnchor | null;
    current: number;
    onpick: (priority: number) => void;
    returnFocus?: HTMLElement | null;
  }
  let { open = $bindable(), anchor, current, onpick, returnFocus = null }: Props = $props();

  const TONES = ['text-danger', 'text-warning', 'text-accent', 'text-faint'];

  const items = $derived<MenuItem[]>(
    [1, 2, 3, 4].map((p) => ({
      label: PRIORITY_NAMES[p] ?? `Priority ${p}`,
      icon: Flag,
      iconClass: TONES[p - 1],
      shortcut: String(p),
      checked: current === p,
      onselect: () => onpick(p),
    })),
  );

  function pick(priority: number) {
    open = false;
    onpick(priority);
  }
</script>

{#if viewport.isPhone}
  <Sheet {open} title="Priority" onclose={() => (open = false)} closable>
    <div class="flex flex-col" role="listbox" aria-label="Priority">
      {#each items as item, i (item.label)}
        <button
          type="button"
          role="option"
          aria-selected={item.checked}
          onclick={() => pick(i + 1)}
          class="flex h-12 w-full cursor-pointer items-center gap-3 rounded-sm px-3 text-left text-ink transition-colors duration-(--dur) ease-out hover:bg-hover"
        >
          {#if item.icon}<item.icon class="size-5 shrink-0 {item.iconClass ?? ''}" />{/if}
          <span class="flex-1">{item.label}</span>
          {#if item.checked}<Check class="size-4 text-accent" />{/if}
        </button>
      {/each}
    </div>
  </Sheet>
{:else}
  <Menu bind:open {anchor} {items} label="Priority" {returnFocus} />
{/if}
