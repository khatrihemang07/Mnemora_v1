<!--
  NavItem: sidebar row. In the rail it shows only the icon, with a 600ms tooltip (N17-A) unless a
  flyout replaces it (`tooltip={false}`). `current` bolds a row whose page is open while its
  children carry the active tint; `chevron` adds the fold button of a nested tree at the end.
-->
<script lang="ts">
  import type { Component } from 'svelte';
  import ChevronRight from '~icons/lucide/chevron-right';
  import type { ModuleManifest } from '../lib/router';
  import Kbd from '../lib/ui/Kbd.svelte';
  import Tooltip from '../lib/ui/Tooltip.svelte';
  import ModuleBadge from './ModuleBadge.svelte';

  interface Props {
    label: string;
    icon: Component<{ class?: string }>;
    rail: boolean;
    href?: string;
    active?: boolean;
    shortcut?: string;
    current?: boolean;
    accentVar?: string;
    /** The Module whose badge (a count) the row shows, if it has one. */
    badgeOf?: ModuleManifest;
    tooltip?: boolean;
    chevron?: { open: boolean; label: string; onclick: () => void };
    onclick?: (e: MouseEvent) => void;
    oncontextmenu?: (e: MouseEvent) => void;
  }
  let {
    label,
    icon: Icon,
    rail,
    href,
    active = false,
    shortcut,
    current = false,
    accentVar,
    badgeOf,
    tooltip = true,
    chevron,
    onclick,
    oncontextmenu,
  }: Props = $props();

  const cls = $derived(
    `flex h-(--row) min-h-8 w-full cursor-pointer items-center gap-2.5 rounded-sm text-left transition-colors duration-(--dur) ease-out hover:bg-hover
     ${rail ? 'justify-center' : chevron ? 'pr-8 pl-2.5' : 'px-2.5'} ${active ? 'bg-active font-semibold text-ink' : current ? 'font-semibold text-ink' : 'text-ink'}`,
  );
</script>

{#snippet row(extra: Record<string, unknown>)}
  {#if href}
    <a
      {href}
      {...extra}
      class={cls}
      aria-label={rail ? label : undefined}
      aria-current={active || current ? 'page' : undefined}
      {onclick}
      {oncontextmenu}
    >
      {@render inner()}
    </a>
  {:else}
    <button type="button" {...extra} class={cls} aria-label={rail ? label : undefined} {onclick} {oncontextmenu}>
      {@render inner()}
    </button>
  {/if}
{/snippet}

{#snippet inner()}
  <span style:color={accentVar ? `var(${accentVar})` : 'var(--muted)'} class="relative grid shrink-0 place-items-center">
    <Icon class="size-[1.15rem]" />
    {#if rail && badgeOf?.useBadge}<ModuleBadge manifest={badgeOf} placement="corner" />{/if}
  </span>
  {#if !rail}
    <span class="min-w-0 flex-1 truncate">{label}</span>
    {#if badgeOf?.useBadge}<ModuleBadge manifest={badgeOf} placement="inline" />{/if}
    {#if shortcut}<Kbd keys={shortcut} />{/if}
  {/if}
{/snippet}

{#if rail && tooltip}
  <Tooltip {label} {shortcut} side="right">
    {#snippet trigger({ props })}{@render row(props)}{/snippet}
  </Tooltip>
{:else if chevron && !rail}
  <div class="relative">
    {@render row({})}
    <button
      type="button"
      aria-label={chevron.label}
      aria-expanded={chevron.open}
      onclick={chevron.onclick}
      class="absolute top-1/2 right-1.5 grid size-5 -translate-y-1/2 cursor-pointer place-items-center rounded-sm text-faint
        transition-colors duration-(--dur) ease-out hover:bg-active hover:text-ink"
    >
      <ChevronRight class="size-3.5 transition-transform duration-(--dur) ease-out {chevron.open ? 'rotate-90' : ''}" />
    </button>
  </div>
{:else}
  {@render row({})}
{/if}
