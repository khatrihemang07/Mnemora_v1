<!-- CommandPalette: ⌘K list of actions on Bits UI Command. Shows shortcuts (N16-A). -->
<script lang="ts">
  import { Command, Dialog } from 'bits-ui';
  import type { Component } from 'svelte';
  import Kbd from './Kbd.svelte';

  export interface PaletteItem {
    id: string;
    label: string;
    group: string;
    icon?: Component<{ class?: string }>;
    shortcut?: string;
    run: () => void;
  }
  interface Props {
    open: boolean;
    items: PaletteItem[];
    onclose: () => void;
  }
  let { open, items, onclose }: Props = $props();

  const groups = $derived([...new Set(items.map((i) => i.group))]);
</script>

<Dialog.Root {open} onOpenChange={(o) => !o && onclose()}>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-40 bg-scrim" />
    <Dialog.Content
      aria-label="Command palette"
      class="fixed top-[18vh] left-1/2 z-50 w-[min(92vw,34rem)] -translate-x-1/2 overflow-hidden rounded-md bg-pop shadow-pop"
    >
      <Dialog.Title class="sr-only">Command palette</Dialog.Title>
      <Command.Root loop>
        <Command.Input
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
                {#each items.filter((i) => i.group === g) as item (item.id)}
                  <Command.Item
                    value={item.label}
                    onSelect={() => {
                      onclose();
                      item.run();
                    }}
                    class="flex h-9 cursor-pointer items-center gap-2.5 rounded-sm px-2.5 text-ink select-none data-selected:bg-hover"
                  >
                    {#if item.icon}<item.icon class="size-4 text-muted" />{/if}
                    <span class="flex-1">{item.label}</span>
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
