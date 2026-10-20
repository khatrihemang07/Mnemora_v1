<!-- NavItem: sidebar row. In the rail it shows only the icon, with a 600ms tooltip (N17-A). -->
<script lang="ts">
  import type { Component } from 'svelte';
  import Kbd from '../lib/ui/Kbd.svelte';
  import Tooltip from '../lib/ui/Tooltip.svelte';

  interface Props {
    label: string;
    icon: Component<{ class?: string }>;
    rail: boolean;
    href?: string;
    active?: boolean;
    shortcut?: string;
    accentVar?: string;
    onclick?: () => void;
  }
  let { label, icon: Icon, rail, href, active = false, shortcut, accentVar, onclick }: Props = $props();

  const cls = $derived(
    `flex h-(--row) min-h-8 w-full cursor-pointer items-center gap-2.5 rounded-sm text-left transition-colors duration-(--dur) ease-out hover:bg-hover
     ${rail ? 'justify-center' : 'px-2.5'} ${active ? 'bg-active font-semibold text-ink' : 'text-ink'}`,
  );
</script>

{#snippet row(extra: Record<string, unknown>)}
  {#if href}
    <a {href} {...extra} class={cls} aria-label={rail ? label : undefined} aria-current={active ? 'page' : undefined}>
      {@render inner()}
    </a>
  {:else}
    <button type="button" {...extra} class={cls} aria-label={rail ? label : undefined} {onclick}>
      {@render inner()}
    </button>
  {/if}
{/snippet}

{#snippet inner()}
  <span style:color={accentVar ? `var(${accentVar})` : 'var(--muted)'} class="grid shrink-0 place-items-center">
    <Icon class="size-[1.15rem]" />
  </span>
  {#if !rail}
    <span class="min-w-0 flex-1 truncate">{label}</span>
    {#if shortcut}<Kbd keys={shortcut} />{/if}
  {/if}
{/snippet}

{#if rail}
  <Tooltip {label} {shortcut} side="right">
    {#snippet trigger({ props })}{@render row(props)}{/snippet}
  </Tooltip>
{:else}
  {@render row({})}
{/if}
