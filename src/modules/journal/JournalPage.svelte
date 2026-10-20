<!-- JournalPage: the feed of dated Pages (newest first) with the composer docked below. -->
<script lang="ts">
  import { createInfiniteQuery, createQuery } from '@tanstack/svelte-query';
  import ArrowUp from '~icons/lucide/arrow-up';
  import NotebookPen from '~icons/lucide/notebook-pen';
  import { readCore } from '../../lib/settings/core';
  import { settingsQuery } from '../../lib/settings/queries';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import VirtualList from '../../lib/ui/VirtualList.svelte';
  import ComposerDock from './ComposerDock.svelte';
  import { localDateString, showTodayPill } from './dates';
  import { buildFeed, type FeedItem } from './feed';
  import PageSection from './PageSection.svelte';
  import { pagesQuery } from './queries';

  /** Guesses before a row is measured: a header plus a couple of Entries. */
  const PAGE_ESTIMATE_PX = 70;
  const ENTRY_ESTIMATE_PX = 90;
  const MONTH_ESTIMATE_PX = 52;

  const pages = createInfiniteQuery(() => pagesQuery);
  const settings = createQuery(() => settingsQuery);
  const timeFormat = $derived(readCore(settings.data?.values).timeFormat);
  const today = localDateString(new Date());
  const feed = $derived(buildFeed(pages.data?.pages.flat() ?? [], today));

  let list = $state<ReturnType<typeof VirtualList<FeedItem>> | null>(null);
  let pill = $state(false);

  const estimate = (item: FeedItem) =>
    item.kind === 'month'
      ? MONTH_ESTIMATE_PX
      : PAGE_ESTIMATE_PX + item.page.entry_count * ENTRY_ESTIMATE_PX;

  function loadOlder() {
    if (pages.hasNextPage && !pages.isFetchingNextPage) void pages.fetchNextPage();
  }
</script>

<div class="flex h-full flex-col">
  <!-- Phone shows a large title here; on desktop PageHeader renders nothing and the box collapses. -->
  <div class="shrink-0 px-(--pad) pt-(--pad) empty:hidden"><PageHeader title="Journal" /></div>
  <div class="relative min-h-0 flex-1">
    {#if pages.isPending}
      <div class="mx-auto flex max-w-[42.5rem] flex-col gap-3 px-(--pad) py-4">
        <Skeleton class="h-6 w-48" />
        <Skeleton class="h-16 w-full" />
      </div>
    {:else if pages.isError}
      <div class="mx-auto max-w-[42.5rem] px-(--pad) py-4">
        <ErrorBanner error={pages.error} onretry={() => pages.refetch()} />
      </div>
    {:else if feed.length === 0}
      <div class="mx-auto max-w-[42.5rem] px-(--pad)">
        <EmptyState
          icon={NotebookPen}
          hue="--hue-journal"
          title="A blank page"
          description="Your first Entry starts here. Write a line, or a lot."
        >
          {#snippet action()}
            <span class="text-xs text-faint">Start typing in the box below</span>
          {/snippet}
        </EmptyState>
      </div>
    {:else}
      <VirtualList
        bind:this={list}
        items={feed}
        getKey={(item) => item.key}
        estimateSize={estimate}
        onnearend={loadOlder}
        onviewport={(top, height) => (pill = showTodayPill(top, height))}
        class="h-full px-(--pad)"
        columnClass="mx-auto max-w-[42.5rem]"
      >
        {#snippet row(item)}
          {#if item.kind === 'month'}
            <div
              class="flex items-center gap-2.5 pt-7 pb-1 text-[11px] font-semibold tracking-[0.08em] text-faint uppercase after:h-px after:flex-1 after:bg-line"
            >
              {item.label}
            </div>
          {:else}
            <PageSection page={item.page} {today} {timeFormat} />
          {/if}
        {/snippet}
      </VirtualList>
      {#if pill}
        <button
          type="button"
          class="absolute top-3 left-1/2 z-10 inline-flex -translate-x-1/2 items-center gap-1.5 rounded-full bg-ink px-3.5 py-1.5 text-xs font-semibold text-bg shadow-(--elev-pop)"
          onclick={() => list?.scrollToTop()}
        >
          <ArrowUp class="size-3.5" /> Today
        </button>
      {/if}
    {/if}
  </div>
  <ComposerDock />
</div>
