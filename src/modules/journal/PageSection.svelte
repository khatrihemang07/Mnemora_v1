<!--
  PageSection: one dated Page in the feed: its PageHeading (pinned by the feed while you read the
  Page) and its Entries separated by hairlines. Today's Page, when empty, says the composer below is ready.
-->
<script lang="ts">
  import type { Page } from '../../lib/ipc/bindings';
  import type { TimeFormat } from '../../lib/time/format';
  import EntryRow from './EntryRow.svelte';
  import PageHeading from './PageHeading.svelte';

  let {
    page,
    today,
    timeFormat,
    summary,
  }: { page: Page; today: string; timeFormat: TimeFormat; summary: boolean } = $props();
</script>

<section>
  <PageHeading {page} {today} {summary} />
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
