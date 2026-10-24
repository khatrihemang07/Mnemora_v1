<!--
  PageHeading: a Page's header: written date in the display serif, the year when it is not this
  year, a Today/Yesterday badge and a desktop-only summary (hidden when Page summaries is off). Used in the feed and as its pinned copy.
  Flashes when the calendar just pointed at its day.
-->
<script lang="ts">
  import type { Page } from '../../lib/ipc/bindings';
  import { dayHue, pageHeading, pageSummary } from './dates';
  import { useNav } from './navigation.svelte';

  let { page, today, summary }: { page: Page; today: string; summary: boolean } = $props();
  const nav = useNav();
  const heading = $derived(pageHeading(page.date, today));
</script>

<header
  class="flex items-baseline gap-2.5 bg-bg pt-3 pb-2 {nav.flashPage === page.date ? 'flash-header' : ''}"
>
  <h2
    class="font-display text-2xl leading-[1.15]"
    style="font-weight: var(--display-weight); letter-spacing: var(--display-track)"
  >
    {heading.title}
  </h2>
  {#if heading.year}<span class="font-display text-lg leading-normal text-faint">{heading.year}</span>{/if}
  {#if heading.badge}
    <span
      style="--badge-hue: var({dayHue(page.date, today)})"
      class="rounded-full bg-[color-mix(in_oklch,var(--badge-hue)_14%,transparent)] px-2 py-px text-2xs font-semibold text-(--badge-hue)"
    >
      {heading.badge}
    </span>
  {/if}
  {#if summary && page.entry_count > 0}
    <span class="ml-auto hidden text-xs text-faint tabular-nums desktop:block">
      {pageSummary(page.entry_count, page.word_count)}
    </span>
  {/if}
</header>
