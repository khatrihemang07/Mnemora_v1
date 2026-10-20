<!--
  PageSection: one dated Page in the feed: a sticky header (written date in the display serif, a
  Today/Yesterday badge, the year when it is not this year, a desktop-only summary) and its Entries
  separated by hairlines. Today's Page, when empty, says the composer below is ready.
-->
<script lang="ts">
  import type { Page } from '../../lib/ipc/bindings';
  import type { TimeFormat } from '../../lib/time/format';
  import { pageHeading, pageSummary } from './dates';
  import EntryRow from './EntryRow.svelte';

  let { page, today, timeFormat }: { page: Page; today: string; timeFormat: TimeFormat } = $props();
  const heading = $derived(pageHeading(page.date, today));
</script>

<section>
  <header class="sticky top-0 z-[2] flex items-baseline gap-2.5 bg-bg pt-3 pb-2">
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
  {#if page.entries.length === 0}
    <p class="pt-1.5 pb-0.5 text-[0.84rem] text-faint">Nothing yet today. The box below is ready.</p>
  {:else}
    <div>
      {#each page.entries as entry (entry.id)}
        <EntryRow {entry} {timeFormat} />
      {/each}
    </div>
  {/if}
</section>
