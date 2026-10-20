<!--
  Menu: a list of actions that opens beside a button or at a point (right-click, long-press). One
  Component for both, controlled: the caller owns `open` and says where with `anchor` (an element or
  a screen point). Bits UI gives arrow keys, typeahead, Esc and outside-click. Dismissing returns
  focus to the anchor element (or `returnFocus`); picking an item leaves focus to whatever the action opens.
-->
<script lang="ts" module>
  import type { Component } from 'svelte';

  export interface MenuItem {
    label: string;
    icon?: Component<{ class?: string }>;
    /** A hint such as `⌫`, shown at the right edge. */
    shortcut?: string;
    /** Red: for destructive actions. */
    danger?: boolean;
    /** Draws a hairline above the item. */
    separatorBefore?: boolean;
    onselect: () => void;
  }

  /** What the menu hangs off: a button, or a point on the screen. */
  export type MenuAnchor = HTMLElement | { x: number; y: number };
</script>

<script lang="ts">
  import { DropdownMenu } from 'bits-ui';
  import Kbd from './Kbd.svelte';

  interface Props {
    open: boolean;
    anchor: MenuAnchor | null;
    items: MenuItem[];
    /** Names the menu for screen readers. */
    label: string;
    side?: 'top' | 'bottom';
    /** Where focus goes on dismissal when the anchor is a point rather than an element. */
    returnFocus?: HTMLElement | null;
  }
  let {
    open = $bindable(),
    anchor,
    items,
    label,
    side = 'bottom',
    returnFocus = null,
  }: Props = $props();

  let picked = false;

  // Bits positions against anything that can measure itself; a point is a zero-size rectangle.
  const measurable = $derived(
    anchor === null
      ? null
      : anchor instanceof HTMLElement
        ? anchor
        : {
            getBoundingClientRect: () =>
              DOMRect.fromRect({ x: anchor.x, y: anchor.y, width: 0, height: 0 }),
          },
  );

  function onCloseAutoFocus(e: Event) {
    e.preventDefault();
    if (!picked) (anchor instanceof HTMLElement ? anchor : returnFocus)?.focus();
    picked = false;
  }
</script>

<DropdownMenu.Root bind:open>
  <DropdownMenu.Portal>
    <DropdownMenu.Content
      customAnchor={measurable}
      {side}
      align="start"
      sideOffset={4}
      collisionPadding={8}
      aria-label={label}
      {onCloseAutoFocus}
      class="z-50 min-w-48 rounded-md bg-pop p-1.25 text-[0.8125rem] shadow-pop outline-none"
    >
      {#each items as item (item.label)}
        {#if item.separatorBefore}
          <DropdownMenu.Separator class="my-1 h-px bg-line" />
        {/if}
        <DropdownMenu.Item
          textValue={item.label}
          onSelect={() => {
            picked = true;
            item.onselect();
          }}
          class="flex min-h-9 cursor-pointer items-center gap-2.5 rounded-sm px-2.5 py-1.5 select-none data-highlighted:bg-hover
            min-[720px]:min-h-8 {item.danger ? 'text-danger' : 'text-ink'}"
        >
          {#if item.icon}<item.icon class="size-4 shrink-0 {item.danger ? '' : 'text-muted'}" />{/if}
          <span class="flex-1">{item.label}</span>
          {#if item.shortcut}<Kbd keys={item.shortcut} />{/if}
        </DropdownMenu.Item>
      {/each}
    </DropdownMenu.Content>
  </DropdownMenu.Portal>
</DropdownMenu.Root>
