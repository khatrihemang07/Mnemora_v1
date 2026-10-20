<!--
  JournalPage: the feed of dated Pages (newest first) with the composer docked below. Future Pages
  sit above Today; the Journal opens scrolled to Today, with an "↑ Upcoming (n)" pill to go back up
  and an "↑ Today" pill once you are far below. The header calendar button (⌘G) opens the
  JournalCalendar. `JournalNav` (context) lets Entry actions and the calendar scroll and flash.
-->
<script lang="ts">
  import { createInfiniteQuery, createQuery } from '@tanstack/svelte-query';
  import { onDestroy, tick } from 'svelte';
  import ArrowUp from '~icons/lucide/arrow-up';
  import CalendarDays from '~icons/lucide/calendar-days';
  import NotebookPen from '~icons/lucide/notebook-pen';
  import { hasMod, shortcutLabel } from '../../lib/platform';
  import { readCore } from '../../lib/settings/core';
  import { settingsQuery } from '../../lib/settings/queries';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import KeyboardBar from '../../lib/ui/KeyboardBar.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import VirtualList from '../../lib/ui/VirtualList.svelte';
  import { viewport } from '../../lib/viewport.svelte';
  import ComposerDock from './ComposerDock.svelte';
  import { localDateString, showTodayPill, showUpcomingPill } from './dates';
  import { Editing, provideEditing } from './editing.svelte';
  import { buildFeed, type FeedItem, openIndex, pageAt, pageIndex, upcomingCount } from './feed';
  import JournalCalendar from './JournalCalendar.svelte';
  import { JournalNav, provideNav } from './navigation.svelte';
  import PageHeading from './PageHeading.svelte';
  import PageSection from './PageSection.svelte';
  import { pagesQuery } from './queries';
  import { slashValues } from './slash-values';
  import UnsavedEntryDialog from './UnsavedEntryDialog.svelte';

  /** Guesses before a row is measured: a header plus a couple of Entries. */
  const PAGE_ESTIMATE_PX = 70;
  const ENTRY_ESTIMATE_PX = 90;
  const MONTH_ESTIMATE_PX = 52;

  const pages = createInfiniteQuery(() => pagesQuery);
  const settings = createQuery(() => settingsQuery);
  const core = $derived(readCore(settings.data?.values));
  const timeFormat = $derived(core.timeFormat);

  // Which Entry is open for editing, which editor the phone bar drives, and the slash menu formats.
  const editing = new Editing();
  editing.slashValues = () => slashValues(new Date(), core.dateFormat, core.timeFormat);
  provideEditing(editing);
  const today = localDateString(new Date());
  const nav = new JournalNav(today);
  provideNav(nav);
  nav.scrollTo = scrollTo;
  onDestroy(() => nav.dispose());
  $effect(() => {
    nav.weekStart = core.weekStart;
  });

  const loaded = $derived(pages.data?.pages.flat() ?? []);
  const feed = $derived(buildFeed(loaded, today));
  const upcoming = $derived(upcomingCount(loaded, today));
  /** Where the Journal opens: Today's Page when future Pages sit above it. */
  const open = $derived(openIndex(feed, today));

  let list = $state<ReturnType<typeof VirtualList<FeedItem>> | null>(null);
  let scrolled = $state({ top: 0, height: 0 });
  let opened = false;
  let calendarOpen = $state(false);
  let calendarAnchor = $state<HTMLButtonElement | null>(null);
  let calendarHighlight = $state(today);

  // Where Today's area starts: the pills and the opening scroll are measured from here.
  const todayTop = $derived.by(() => {
    void feed;
    return list && open > 0 ? list.offsetOf(open) : 0;
  });
  const pill = $derived(showTodayPill(scrolled.top, scrolled.height, todayTop));
  const upcomingPill = $derived(showUpcomingPill(scrolled.top, scrolled.height, todayTop, upcoming));

  const estimate = (item: FeedItem) =>
    item.kind === 'month'
      ? MONTH_ESTIMATE_PX
      : PAGE_ESTIMATE_PX + item.page.entry_count * ENTRY_ESTIMATE_PX;

  function loadOlder() {
    if (pages.hasNextPage && !pages.isFetchingNextPage) void pages.fetchNextPage();
  }

  // Open on Today, not the top, once any future Pages above it are loaded.
  $effect(() => {
    if (opened || !list || feed.length === 0) return;
    const last = loaded.at(-1);
    if (last && last.date > today && pages.hasNextPage) {
      loadOlder(); // still inside the Upcoming Pages
      return;
    }
    opened = true;
    if (open > 0) void tick().then(() => list?.scrollToIndex(open));
  });

  /** Waits for what the feed does after a data change: Svelte's flush, then Query's notification. */
  async function settle() {
    await tick();
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  }

  async function nextFrames(n: number) {
    for (let i = 0; i < n; i++) await new Promise((resolve) => requestAnimationFrame(resolve));
  }

  /**
   * Scrolls to the Page for `date` (loading older Pages until it is there) and flashes its header,
   * or the Entry `entryId` on it. Gives up quietly when no Page has that date.
   */
  async function scrollTo(date: string, entryId?: string) {
    await settle();
    while (pageIndex(feed, date) < 0 && pages.hasNextPage && (loaded.at(-1)?.date ?? '') > date) {
      const before = loaded.length;
      await pages.fetchNextPage();
      await settle();
      if (loaded.length === before) break;
    }
    const index = pageIndex(feed, date);
    if (index < 0 || !list) return;
    list.scrollToIndex(index);
    if (!entryId) {
      nav.flashPageFor(date);
      return;
    }
    // The Entry's row renders once the jump lands; bring it to mid-screen, clear of the pinned header.
    for (let i = 0; i < 30; i++) {
      await nextFrames(1);
      const row = document.querySelector(`[data-entry-id="${CSS.escape(entryId)}"]`);
      if (row) {
        row.scrollIntoView({ block: 'center' });
        break;
      }
    }
    nav.flashEntryFor(entryId);
  }

  function openCalendar() {
    const at = list ? list.indexAt(scrolled.top + 1) : 0;
    calendarHighlight = pageAt(feed, at) ?? today;
    calendarOpen = !calendarOpen;
  }

  function onkeydown(e: KeyboardEvent) {
    if (!hasMod(e) || e.shiftKey || (e.code !== 'KeyG' && e.key !== 'g')) return;
    e.preventDefault();
    openCalendar();
  }
</script>

<svelte:window {onkeydown} />

{#snippet headerActions()}
  <IconButton
    bind:ref={calendarAnchor}
    icon={CalendarDays}
    label="Go to date"
    shortcut={shortcutLabel('G')}
    aria-haspopup="dialog"
    aria-expanded={calendarOpen}
    onclick={openCalendar}
  />
{/snippet}

<div class="flex h-full flex-col">
  <!-- Phone shows a large title here; on desktop PageHeader renders nothing and the box collapses. -->
  <div class="shrink-0 px-(--pad) pt-(--pad) empty:hidden">
    <PageHeader title="Journal" actions={headerActions} />
  </div>
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
        onviewport={(top, height) => (scrolled = { top, height })}
        isSticky={(item) => item.kind === 'page'}
        class="h-full px-(--pad)"
        columnClass="mx-auto max-w-[42.5rem]"
      >
        {#snippet pinned(item)}
          {#if item.kind === 'page'}<PageHeading page={item.page} {today} />{/if}
        {/snippet}
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
      <!-- The feed fades out above the composer instead of ending on a hard edge. -->
      <div
        class="pointer-events-none absolute inset-x-0 bottom-0 h-6 bg-linear-to-t from-bg to-transparent"
      ></div>
      <!-- Below the pinned Page header, so it never hides the date. -->
      {#if pill}
        <button
          type="button"
          class="absolute top-[4.25rem] left-1/2 z-10 inline-flex -translate-x-1/2 items-center gap-1.5 rounded-full bg-ink px-3.5 py-1.5 text-xs font-semibold text-bg shadow-(--elev-pop)"
          onclick={() => list?.scrollToIndex(open, true)}
        >
          <ArrowUp class="size-3.5" /> Today
        </button>
      {:else if upcomingPill}
        <button
          type="button"
          class="absolute top-[4.25rem] left-1/2 z-10 inline-flex -translate-x-1/2 items-center gap-1.5 rounded-full border border-line bg-pop px-3.5 py-1.5 text-xs font-semibold text-(--hue-journal) shadow-(--elev-pop)"
          onclick={() => list?.scrollToIndex(0, true)}
        >
          <ArrowUp class="size-3.5" /> Upcoming ({upcoming})
        </button>
      {/if}
    {/if}
  </div>
  <ComposerDock />
</div>
<KeyboardBar
  visible={viewport.isPhone && editing.active !== null}
  onaction={(command) => editing.active?.run(command)}
/>
<UnsavedEntryDialog />
<JournalCalendar
  bind:open={calendarOpen}
  anchor={calendarAnchor}
  {today}
  weekStart={core.weekStart}
  highlight={calendarHighlight}
  onshow={(date) => void scrollTo(date)}
  onwrite={(date) => nav.writeOn(date)}
/>
