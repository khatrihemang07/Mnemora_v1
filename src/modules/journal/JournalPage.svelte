<!-- JournalPage: the feed of dated Pages (newest first) with the composer docked below. -->
<script lang="ts">
  import { createInfiniteQuery, createQuery } from '@tanstack/svelte-query';
  import NotebookPen from '~icons/lucide/notebook-pen';
  import { readCore } from '../../lib/settings/core';
  import { settingsQuery } from '../../lib/settings/queries';
  import Button from '../../lib/ui/Button.svelte';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import ComposerDock from './ComposerDock.svelte';
  import { localDateString, pageTitle } from './dates';
  import EntryRow from './EntryRow.svelte';
  import { pagesQuery } from './queries';

  const pages = createInfiniteQuery(() => pagesQuery);
  const settings = createQuery(() => settingsQuery);
  const timeFormat = $derived(readCore(settings.data?.values).timeFormat);
  const list = $derived(pages.data?.pages.flat() ?? []);
  const today = localDateString(new Date());
</script>

<div class="flex h-full flex-col">
  <!-- Phone shows a large title here; on desktop PageHeader renders nothing and the box collapses. -->
  <div class="shrink-0 px-(--pad) pt-(--pad) empty:hidden"><PageHeader title="Journal" /></div>
  <div class="min-h-0 flex-1 overflow-y-auto px-(--pad)">
    <div class="mx-auto max-w-[42.5rem] pb-6">
      {#if pages.isPending}
        <div class="flex flex-col gap-3 py-4">
          <Skeleton class="h-6 w-48" />
          <Skeleton class="h-16 w-full" />
        </div>
      {:else if pages.isError}
        <div class="py-4">
          <ErrorBanner error={pages.error} onretry={() => pages.refetch()} />
        </div>
      {:else if list.length === 0}
        <EmptyState
          icon={NotebookPen}
          title="A blank page"
          description="Your first Entry starts here. Write a line, or a lot."
        />
      {:else}
        {#each list as page (page.date)}
          <section class="pt-4">
            <h2
              class="border-b border-line pb-1 font-display text-lg"
              style="font-weight: var(--display-weight)"
            >
              {pageTitle(page.date, today)}
            </h2>
            <div class="divide-y divide-line">
              {#each page.entries as entry (entry.id)}
                <EntryRow {entry} {timeFormat} />
              {/each}
            </div>
          </section>
        {/each}
        {#if pages.hasNextPage}
          <div class="flex justify-center pt-4">
            <Button onclick={() => pages.fetchNextPage()} disabled={pages.isFetchingNextPage}>
              Load older
            </Button>
          </div>
        {/if}
      {/if}
    </div>
  </div>
  <ComposerDock />
</div>
