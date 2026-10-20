<!--
  PageHeading: a Page's header: written date in the display serif, the year when it is not this
  year, a Today/Yesterday badge and a desktop-only summary. Used in the feed and as its pinned copy.
  Flashes when the calendar just pointed at its day.
-->
<script lang="ts">
  import type { Page } from '../../lib/ipc/bindings';
  import { pageHeading, pageSummary } from './dates';
  import { useNav } from './navigation.svelte';

  let { page, today }: { page: Page; today: string } = $props();
  const nav = useNav();
  const heading = $derived(pageHeading(page.date, today));
</script>

<header
  class="flex items-baseline gap-2.5 bg-bg pt-3 pb-2 {nav.flashPage === page.date ? 'flash-header' : ''}"
>
  <h2
    class="font-display text-[1.45rem] leading-[1.15]"
    style="font-weight: var(--display-weight); letter-spacing: var(--display-track)"
  >
    {heading.title}
  </h2>
  {#if heading.year}<span class="font-display text-[1.1rem] text-faint">{heading.year}</span>{/if}
  {#if heading.badge}
    <span
      class="rounded-full bg-[color-mix(in_oklch,var(--hue-journal)_14%,transparent)] px-2 py-px text-[11px] font-semibold text-(--hue-journal)"
    >
      {heading.badge}
    </span>
  {/if}
  {#if page.entry_count > 0}
    <span class="ml-auto hidden text-xs text-faint tabular-nums min-[720px]:block">
      {pageSummary(page.entry_count, page.word_count)}
    </span>
  {/if}
</header>
