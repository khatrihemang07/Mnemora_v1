<!--
  CommandPalette: ⌘K list of actions on Bits UI Command. Shows shortcuts (N16-A). Filtering is ours
  (`visibleItems`), so an item with a `queryLabel` can offer to act on what was typed
  ("Search Journal for 'river'") and `run` receives it.
-->
<script lang="ts">
  import { Command, Dialog } from 'bits-ui';
  import type { Component } from 'svelte';
  import Kbd from './Kbd.svelte';
  import { visibleItems } from './palette-filter';

  export interface PaletteItem {
    id: string;
    label: string;
    group: string;
    icon?: Component<{ class?: string }>;
    shortcut?: string;
    /** Makes it a query action: it shows once something is typed, with this label for the text. */
    queryLabel?: (query: string) => string;
    /** `query` is the trimmed text typed in the palette. */
    run: (query: string) => void;
  }
  interface Props {
    open: boolean;
    items: PaletteItem[];
    onclose: () => void;
  }
  let { open, items, onclose }: Props = $props();

  let typed = $state('');
  const shown = $derived(visibleItems(items, typed));
  const groups = $derived([...new Set(shown.map((v) => v.item.group))]);

  // Each opening starts empty.
  $effect(() => {
    if (open) typed = '';
  });
</script>

<Dialog.Root {open} onOpenChange={(o) => !o && onclose()}>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-40 bg-scrim" />
    <Dialog.Content
      aria-label="Command palette"
      class="fixed top-[18vh] left-1/2 z-50 w-[min(92vw,34rem)] -translate-x-1/2 overflow-hidden rounded-md bg-pop shadow-pop"
    >
      <Dialog.Title class="sr-only">Command palette</Dialog.Title>
      <Command.Root loop shouldFilter={false}>
        <Command.Input
          bind:value={typed}
          placeholder="Type a command…"
          class="selectable h-12 w-full border-b border-line bg-transparent px-4 text-ink placeholder:text-faint"
        />
        <Command.List class="max-h-80 overflow-y-auto p-1.5">
          <Command.Empty class="px-3 py-6 text-center text-muted">Nothing matches.</Command.Empty>
          {#each groups as g (g)}
            <Command.Group>
              <Command.GroupHeading class="px-2.5 pt-2 pb-1 text-xs font-medium text-faint">
                {g}
              </Command.GroupHeading>
              <Command.GroupItems>
                {#each shown.filter((v) => v.item.group === g) as { item, label } (item.id)}
                  <Command.Item
                    value={item.id}
                    onSelect={() => {
                      onclose();
                      item.run(typed.trim());
                    }}
                    class="flex h-9 cursor-pointer items-center gap-2.5 rounded-sm px-2.5 text-ink select-none data-selected:bg-hover"
                  >
                    {#if item.icon}<item.icon class="size-4 text-muted" />{/if}
                    <span class="flex-1">{label}</span>
                    {#if item.shortcut}<Kbd keys={item.shortcut} />{/if}
                  </Command.Item>
                {/each}
              </Command.GroupItems>
            </Command.Group>
          {/each}
        </Command.List>
      </Command.Root>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
