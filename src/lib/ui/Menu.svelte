<!--
  Menu: a list of actions that opens beside a button or at a point (right-click, long-press). One
  Component for both, controlled: the caller owns `open` and says where with `anchor` (an element or
  a screen point). Bits UI gives arrow keys, typeahead, Esc and outside-click. Dismissing returns
  focus to the anchor element (or `returnFocus`); picking an item leaves focus to whatever the action opens.
  An optional `heading` names a choice menu ("Sort Lists by"); `checked` ticks the current choice.
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
    /** A colour class for the icon, e.g. `text-danger` (ignored when `danger`). */
    iconClass?: string;
    /** Marks the current choice with a tick at the right edge. */
    checked?: boolean;
    /** Shown but not available yet. */
    disabled?: boolean;
    /** Draws a hairline above the item. */
    separatorBefore?: boolean;
    onselect: () => void;
  }

  /** What the menu hangs off: a button, or a point on the screen. */
  export type MenuAnchor = HTMLElement | { x: number; y: number };
</script>

<script lang="ts">
  import { DropdownMenu } from 'bits-ui';
  import Check from '~icons/lucide/check';
  import { closeOnBack } from '../router/back-button.svelte';
  import Kbd from './Kbd.svelte';
  import { afterRelease } from './pointer-held';

  interface Props {
    open: boolean;
    anchor: MenuAnchor | null;
    items: MenuItem[];
    /** Names the menu for screen readers. */
    label: string;
    /** A small title above the items, for a menu that picks one of several. */
    heading?: string;
    side?: 'top' | 'bottom';
    /** Where focus goes on dismissal when the anchor is a point rather than an element. */
    returnFocus?: HTMLElement | null;
  }
  let {
    open = $bindable(),
    anchor,
    items,
    label,
    heading,
    side = 'bottom',
    returnFocus = null,
  }: Props = $props();

  closeOnBack(() => open, () => (open = false));

  let picked = false;

  // A menu opened by a long-press appears under a finger that is still down; that finger's release
  // (and the click after it) must neither pick an item nor count as a tap outside.
  let settled = $state(true);
  $effect(() => {
    if (!open) return;
    settled = false;
    return afterRelease(() => (settled = true));
  });

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
      onInteractOutside={(e) => !settled && e.preventDefault()}
      class="z-50 min-w-48 rounded-md bg-pop p-1.25 text-sm leading-normal shadow-pop outline-none"
    >
      {#if heading}
        <div class="px-2.5 pt-1 pb-1.5 text-xs font-medium text-faint">{heading}</div>
      {/if}
      {#each items as item (item.label)}
        {#if item.separatorBefore}
          <!-- --line nearly matches --pop in dark themes, so the divider is drawn from the ink colour. -->
          <DropdownMenu.Separator class="my-1 h-px bg-[color-mix(in_oklch,var(--ink)_12%,transparent)]" />
        {/if}
        <DropdownMenu.Item
          textValue={item.label}
          disabled={item.disabled}
          onSelect={(e) => {
            if (!settled) return e.preventDefault();
            picked = true;
            item.onselect();
          }}
          class="flex min-h-9 cursor-pointer items-center gap-2.5 rounded-sm px-2.5 py-1.5 select-none data-highlighted:bg-hover data-disabled:cursor-default data-disabled:opacity-50 data-disabled:data-highlighted:bg-transparent
            desktop:min-h-8 {item.danger ? 'text-danger' : 'text-ink'}"
        >
          {#if item.icon}<item.icon class="size-4 shrink-0 {item.danger ? '' : (item.iconClass ?? 'text-muted')}" />{/if}
          <span class="flex-1">{item.label}</span>
          {#if item.checked}<Check class="size-4 text-accent" />{/if}
          {#if item.shortcut}<span class="phone:hidden"><Kbd keys={item.shortcut} /></span>{/if}
        </DropdownMenu.Item>
      {/each}
    </DropdownMenu.Content>
  </DropdownMenu.Portal>
</DropdownMenu.Root>
