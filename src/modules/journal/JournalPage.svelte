<!--
  JournalPage: the feed of dated Pages (newest first) with the composer docked below. Future Pages
  sit above Today; the Journal opens scrolled to Today, with an "↑ Upcoming (n)" pill to go back up
  and an "↑ Today" pill once you are far below. The header calendar button (⌘G) opens the
  JournalCalendar. `JournalNav` (context) lets Entry actions and the calendar scroll and flash.
  The header search button (⌘F, or ⌘K's "Search Journal") lays SearchView over the feed, which stays
  mounted underneath so its scroll place survives; opening a result parks the search (a "Back to
  results" pill, ⌘F and Android Back return to it), and every other way out clears it. `EntryFocus`
  (context) is the keyboard ring across Entries: it asks this page to scroll an Entry into view and
  to load older Pages. Requests from outside (⌘N, ⌘K, the Home card) arrive through `launcher`. The
  Journal Preferences (font, text width, Page summaries) and the spellcheck Device setting are read
  here and handed down: the width as the `--journal-width` custom property every part of the column uses.
-->
<script lang="ts">
  import { createInfiniteQuery, createQuery } from '@tanstack/svelte-query';
  import { onDestroy, tick, untrack } from 'svelte';
  import ArrowUp from '~icons/lucide/arrow-up';
  import ArrowLeft from '~icons/lucide/arrow-left';
  import CalendarDays from '~icons/lucide/calendar-days';
  import NotebookPen from '~icons/lucide/notebook-pen';
  import Search from '~icons/lucide/search';
  import X from '~icons/lucide/x';
  import type { Hit } from '../../lib/ipc/bindings';
  import { hasMod, shortcutLabel } from '../../lib/platform';
  import { readCore } from '../../lib/settings/core';
  import { interceptBack } from '../../lib/router';
  import { deviceQuery, settingsQuery } from '../../lib/settings/queries';
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
  import { feedEntries } from './focus-nav';
  import { EntryFocus, provideFocus } from './focus.svelte';
  import JournalCalendar from './JournalCalendar.svelte';
  import { launcher, type JournalRequest } from './launcher.svelte';
  import { JournalNav, provideNav } from './navigation.svelte';
  import PageHeading from './PageHeading.svelte';
  import PageSection from './PageSection.svelte';
  import { pagesQuery } from './queries';
  import { JournalSearch, searchLauncher } from './search.svelte';
  import { columnWidth, readJournal } from './settings';
  import SearchView from './SearchView.svelte';
  import { slashValues } from './slash-values';
  import UnsavedEntryDialog from './UnsavedEntryDialog.svelte';

  /** Guesses before a row is measured: a header plus a couple of Entries. */
  const PAGE_ESTIMATE_PX = 70;
  const ENTRY_ESTIMATE_PX = 90;
  const MONTH_ESTIMATE_PX = 52;
  /** The command palette finishes handing focus back within this long of closing. */
  const PALETTE_CLOSE_MS = 80;

  const pages = createInfiniteQuery(() => pagesQuery);
  const settings = createQuery(() => settingsQuery);
  const core = $derived(readCore(settings.data?.values));
  const timeFormat = $derived(core.timeFormat);
  const prefs = $derived(readJournal(settings.data?.values));
  const device = createQuery(() => deviceQuery);

  // Which Entry is open for editing, which editor the phone bar drives, and the slash menu formats.
  const editing = new Editing();
  editing.slashValues = () => slashValues(new Date(), core.dateFormat, core.timeFormat);
  provideEditing(editing);
  $effect(() => {
    editing.font = prefs.font;
    editing.spellcheck = device.data?.spellcheck ?? true;
  });
  const today = localDateString(new Date());
  const nav = new JournalNav(today);
  provideNav(nav);
  nav.scrollTo = scrollTo;
  const focus = new EntryFocus();
  focus.today = today;
  focus.entries = () => feedEntries(loaded);
  focus.reveal = revealEntry;
  focus.loadOlder = loadOlderNow;
  provideFocus(focus);
  const search = new JournalSearch();
  let searchView = $state<ReturnType<typeof SearchView> | null>(null);
  onDestroy(() => {
    nav.dispose();
    focus.dispose();
    search.dispose();
  });
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

  // Where Today's area starts: the pills and the opening scroll are measured from here. Row
  // measurements aren't reactive, so it is read again on every scroll, not just when the feed changes.
  const todayTop = $derived.by(() => {
    void feed;
    void scrolled;
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

  // A search asked for from ⌘K (possibly just before this page mounted) opens with its words.
  $effect(() => {
    const query = searchLauncher.pending;
    if (query === null) return;
    searchLauncher.pending = null;
    search.openFor(query);
    // The palette gives focus back to where it was as it closes; take it again once it has.
    setTimeout(() => searchView?.focus(false), PALETTE_CLOSE_MS);
  });

  // While search is open or parked, Back (Android) belongs to it: return to the results, or leave.
  $effect(() => {
    if (!search.active && !search.parked) return;
    return interceptBack(() => {
      const leaving = search.active;
      search.back();
      if (leaving) focusComposerSoon();
    });
  });

  /** Loads the next batch of older Pages and waits until they are in the feed. */
  async function loadOlderNow() {
    if (!pages.hasNextPage || pages.isFetchingNextPage) return;
    await pages.fetchNextPage();
    await settle();
  }

  /**
   * Gives the Entry's row the keyboard focus and scrolls it into view (clear of the pinned header
   * and the composer fade, through the row's scroll margins). A row that is not rendered yet is
   * reached by jumping to its Page first.
   */
  async function revealEntry(id: string) {
    const rowFor = () => document.querySelector<HTMLElement>(`[data-entry-id="${CSS.escape(id)}"]`);
    let row = rowFor();
    if (!row) {
      const date = feedEntries(loaded).find((e) => e.id === id)?.date;
      const index = date ? pageIndex(feed, date) : -1;
      if (index < 0 || !list) return;
      list.scrollToIndex(index);
      for (let i = 0; i < 30 && !row; i++) {
        await nextFrames(1);
        row = rowFor();
      }
    }
    if (!row) return;
    row.focus({ preventScroll: true });
    row.scrollIntoView({ block: 'nearest' });
  }

  function focusComposerSoon() {
    if (!viewport.isPhone) void tick().then(() => nav.focusComposer?.());
  }

  /** A launcher request taken once the page is up: ⌘N / the Home card's Write, or ⌘K's Go to date. */
  async function takeRequest(request: JournalRequest) {
    if (request === 'go-to-date') {
      setTimeout(() => {
        if (!calendarOpen) openCalendar();
      }, PALETTE_CLOSE_MS);
      return;
    }
    // Search steps aside and an open edit is saved, so the composer is the one thing in front of you.
    if (search.active || search.parked) search.close();
    await editing.finisher?.();
    await tick();
    if (list) list.scrollToIndex(open);
    // The palette gives focus back as it closes; take it again once it has.
    setTimeout(() => nav.compose?.(), PALETTE_CLOSE_MS);
  }

  $effect(() => {
    const request = launcher.pending;
    if (request === null) return;
    launcher.pending = null;
    untrack(() => void takeRequest(request));
  });

  /** Opening a result: search steps aside (kept for Back), the feed scrolls to the Entry and flashes it. */
  async function openHit(hit: Hit, scrollTop: number) {
    search.park(scrollTop);
    await scrollTo(hit.page_date, hit.id);
  }

  function toggleSearch() {
    if (!search.active) search.open();
    else {
      search.close();
      focusComposerSoon();
    }
  }

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
    if (search.active) search.close(); // a calendar jump or a toast's Show leaves search
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
    if (!hasMod(e) || e.shiftKey) return;
    if (e.code === 'KeyG' || e.key === 'g') {
      e.preventDefault();
      openCalendar();
    } else if (e.code === 'KeyF' || e.key === 'f') {
      e.preventDefault(); // not the browser's find
      if (search.active) searchView?.focus();
      else search.open();
    }
  }
</script>

<svelte:window {onkeydown} />

{#snippet headerActions()}
  <IconButton
    icon={Search}
    label="Search Journal"
    shortcut={shortcutLabel('F')}
    aria-pressed={search.active}
    onclick={toggleSearch}
  />
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

<div class="flex h-full flex-col" style:--journal-width={columnWidth(prefs.width)}>
  <!-- Phone shows a large title here; on desktop PageHeader renders nothing and the box collapses. -->
  <div class="shrink-0 px-(--pad) pt-(--pad) empty:hidden">
    <PageHeader title="Journal" actions={headerActions} />
  </div>
  <div class="relative min-h-0 flex-1">
    <div class="h-full" inert={search.active}>
    {#if pages.isPending}
      <div class="mx-auto flex max-w-(--journal-width) flex-col gap-3 px-(--pad) py-4">
        <Skeleton class="h-6 w-48" />
        <Skeleton class="h-16 w-full" />
      </div>
    {:else if pages.isError}
      <div class="mx-auto max-w-(--journal-width) px-(--pad) py-4">
        <ErrorBanner error={pages.error} onretry={() => pages.refetch()} />
      </div>
    {:else if feed.length === 0}
      <div class="mx-auto max-w-(--journal-width) px-(--pad)">
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
        columnClass="mx-auto max-w-(--journal-width)"
      >
        {#snippet pinned(item)}
          {#if item.kind === 'page'}<PageHeading page={item.page} {today} summary={prefs.pageSummary} />{/if}
        {/snippet}
        {#snippet row(item)}
          {#if item.kind === 'month'}
            <div
              class="flex items-center gap-2.5 pt-7 pb-1 text-[11px] font-semibold tracking-[0.08em] text-faint uppercase after:h-px after:flex-1 after:bg-line"
            >
              {item.label}
            </div>
          {:else}
            <PageSection page={item.page} {today} {timeFormat} summary={prefs.pageSummary} />
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
    {#if search.active}
      <SearchView
        bind:this={searchView}
        {search}
        {today}
        weekStart={core.weekStart}
        {timeFormat}
        onopen={openHit}
        onclose={focusComposerSoon}
        class="absolute inset-0 z-20"
      />
    {:else if search.parked}
      <div
        class="absolute bottom-8 left-1/2 z-10 inline-flex -translate-x-1/2 items-center rounded-full border border-line bg-pop text-xs font-semibold shadow-(--elev-pop)"
      >
        <button
          type="button"
          class="inline-flex cursor-pointer items-center gap-1.5 rounded-l-full py-1.5 pr-2 pl-3.5 text-(--hue-journal) hover:bg-hover"
          onclick={() => search.open()}
        >
          <ArrowLeft class="size-3.5" /> Back to results
        </button>
        <button
          type="button"
          aria-label="Dismiss search results"
          class="grid cursor-pointer place-items-center rounded-r-full py-1.5 pr-2.5 pl-1.5 text-faint hover:bg-hover hover:text-ink"
          onclick={() => search.close()}
        >
          <X class="size-3.5" />
        </button>
      </div>
    {/if}
  </div>
  <ComposerDock class={search.active ? 'hidden' : ''} />
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
