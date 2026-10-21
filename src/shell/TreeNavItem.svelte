<!--
  TreeNavItem: a Module's sidebar row with its own navigation nested underneath (Todo's views and
  Lists). Expanded sidebar: a chevron folds the tree by the rule in `tree-fold.ts`, and clicking the
  row inside the Module does the same. Icon rail: hovering the icon for 600ms or right-clicking it
  opens the Module's flyout of the same destinations; clicking still goes to the Module.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import type { NavEntry } from '../lib/router';
  import { RAIL_FLYOUT_DELAY_MS, RAIL_FLYOUT_LEAVE_MS, setTimer } from '../lib/timing';
  import Popover from '../lib/ui/Popover.svelte';
  import NavItem from './NavItem.svelte';
  import { moduleContext } from './module-context';
  import { router } from './router';
  import { shell } from './shell-state.svelte';
  import { initialOpen, onNavigate, rowClick, toggle } from './tree-fold';

  interface Props {
    entry: NavEntry;
    rail: boolean;
    shortcut?: string;
  }
  let { entry, rail, shortcut }: Props = $props();

  const navTree = $derived(entry.manifest.navTree);
  const inside = $derived(router.isActive(entry.path));
  const loadedTree = $derived(navTree?.tree());
  const loadedFlyout = $derived(navTree?.flyout());

  // Entering unfolds, leaving folds. The first run only records where we are.
  let wasInside: boolean | null = null;
  $effect(() => {
    const now = inside;
    untrack(() => {
      if (wasInside === null) {
        // Starting up: a saved choice stands; with none, being in Todo means unfolded.
        if (shell.treeUnset) shell.treeOpen = initialOpen(null, now);
      } else {
        shell.treeOpen = onNavigate({ open: shell.treeOpen, inside: wasInside }, now).open;
        shell.treeUnset = false;
      }
      wasInside = now;
    });
  });

  function flip() {
    shell.treeOpen = toggle({ open: shell.treeOpen, inside }).open;
    shell.treeUnset = false;
  }

  function onRowClick(e: MouseEvent) {
    flyoutOpen = false;
    if (rail || rowClick({ open: shell.treeOpen, inside }) === 'enter') return;
    e.preventDefault();
    flip();
  }

  // The rail flyout: opens after the hover delay or on right-click, closes shortly after the
  // pointer leaves both the icon and the panel.
  let flyoutOpen = $state(false);
  let anchor = $state<HTMLElement | null>(null);
  let cancelOpen: (() => void) | null = null;
  let cancelClose: (() => void) | null = null;

  function enter() {
    cancelClose?.();
    if (flyoutOpen || cancelOpen) return;
    cancelOpen = setTimer(() => {
      cancelOpen = null;
      flyoutOpen = true;
    }, RAIL_FLYOUT_DELAY_MS);
  }
  function leave() {
    cancelOpen?.();
    cancelOpen = null;
    cancelClose?.();
    cancelClose = setTimer(() => (flyoutOpen = false), RAIL_FLYOUT_LEAVE_MS);
  }
  function onContextMenu(e: MouseEvent) {
    if (!rail) return;
    e.preventDefault();
    cancelClose?.();
    flyoutOpen = true;
  }
</script>

{#if rail}
  <div
    bind:this={anchor}
    role="presentation"
    onpointerenter={enter}
    onpointerleave={leave}
    onpointerdown={() => {
      cancelOpen?.();
      cancelOpen = null;
    }}
  >
    <NavItem
      label={entry.manifest.name}
      icon={entry.manifest.icon}
      href={router.href(entry.path)}
      active={inside}
      accentVar={entry.manifest.accentVar}
      badgeOf={entry.manifest}
      {shortcut}
      rail
      tooltip={false}
      onclick={onRowClick}
      oncontextmenu={onContextMenu}
    />
  </div>
  <Popover
    bind:open={flyoutOpen}
    {anchor}
    label="{entry.manifest.name} views and Lists"
    side="right"
    align="start"
    flush
    autofocus={false}
    restoreFocus={() => {}}
  >
    <div role="presentation" onpointerenter={() => cancelClose?.()} onpointerleave={leave}>
      {#await loadedFlyout then m}
        {#if m}<m.default ctx={moduleContext} onclose={() => (flyoutOpen = false)} />{/if}
      {/await}
    </div>
  </Popover>
{:else}
  <NavItem
    label={entry.manifest.name}
    icon={entry.manifest.icon}
    href={router.href(entry.path)}
    active={inside && !shell.treeOpen}
    current={inside}
    accentVar={entry.manifest.accentVar}
    badgeOf={entry.manifest}
    {shortcut}
    rail={false}
    chevron={{
      open: shell.treeOpen,
      label: shell.treeOpen ? `Collapse ${entry.manifest.name}` : `Expand ${entry.manifest.name}`,
      onclick: flip,
    }}
    onclick={onRowClick}
  />
  {#if shell.treeOpen}
    {#await loadedTree then m}
      {#if m}<m.default ctx={moduleContext} />{/if}
    {/await}
  {/if}
{/if}
