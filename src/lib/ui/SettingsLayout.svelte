<!--
  SettingsLayout (N18-A): section nav + panel on desktop; on the phone a stacked list
  that drills into one section with a Back button.
-->
<script lang="ts">
  import type { Component, Snippet } from 'svelte';
  import ChevronLeft from '~icons/lucide/chevron-left';
  import ChevronRight from '~icons/lucide/chevron-right';
  import PageHeader from './PageHeader.svelte';
  import { viewport } from '../viewport.svelte';

  interface Section {
    id: string;
    label: string;
    icon: Component<{ class?: string }>;
  }
  interface Props {
    sections: Section[];
    active: string | null;
    onselect: (id: string | null) => void;
    children: Snippet;
    footer?: Snippet;
  }
  let { sections, active, onselect, children, footer }: Props = $props();

  const current = $derived(sections.find((s) => s.id === active));
</script>

{#snippet navItem(s: Section, isPhone: boolean)}
  <button
    type="button"
    onclick={() => onselect(s.id)}
    aria-current={active === s.id ? 'page' : undefined}
    class="flex h-(--row) min-h-9 w-full cursor-pointer items-center gap-2.5 rounded-sm px-3 text-left transition-colors duration-(--dur) ease-out hover:bg-hover
      {active === s.id && !isPhone ? 'bg-active font-semibold text-ink' : 'text-ink'}"
  >
    <s.icon class="size-4 text-muted" />
    <span class="flex-1">{s.label}</span>
    {#if isPhone}<ChevronRight class="size-4 text-faint" />{/if}
  </button>
{/snippet}

<div class="relative flex h-full min-h-0">
  {#if viewport.isPhone}
    <div class="selectable h-full w-full overflow-y-auto p-(--pad) pb-24">
      {#if current}
        <button
          type="button"
          onclick={() => onselect(null)}
          class="mb-3 -ml-1 inline-flex h-9 cursor-pointer items-center gap-1 rounded-sm px-2 text-muted hover:bg-hover hover:text-ink"
        >
          <ChevronLeft class="size-4" /> Settings
        </button>
        {@render children()}
      {:else}
        <PageHeader title="Settings" />
        <nav aria-label="Settings sections" class="rounded-md border border-line bg-surface p-1">
          {#each sections as s (s.id)}{@render navItem(s, true)}{/each}
        </nav>
      {/if}
    </div>
  {:else}
    <PageHeader title="Settings" />
    <nav aria-label="Settings sections" class="w-52 shrink-0 overflow-y-auto border-r border-line p-3">
      {#each sections as s (s.id)}{@render navItem(s, false)}{/each}
    </nav>
    <div class="selectable min-w-0 flex-1 overflow-y-auto p-6 pb-24">
      <div class="mx-auto max-w-2xl">{@render children()}</div>
    </div>
  {/if}
  {@render footer?.()}
</div>
