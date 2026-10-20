<!--
  SearchView: the Journal's search, laid over the feed (which stays mounted underneath, so closing
  search puts you back where you were). A field with a search icon and a clear ×, the date chip,
  then live results (150ms after you stop typing). The arrow keys move through the results from the
  field, Enter opens the highlighted one (or the first), Esc clears the words, then closes.
  Opening a result is the page's job (`onopen`). Empty, no-match, loading and error states below.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { onMount, tick } from 'svelte';
  import Search from '~icons/lucide/search';
  import SearchX from '~icons/lucide/search-x';
  import X from '~icons/lucide/x';
  import type { Hit } from '../../lib/ipc/bindings';
  import type { TimeFormat } from '../../lib/time/format';
  import type { WeekStart } from '../../lib/ui/calendar';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import Input from '../../lib/ui/Input.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import { searchQuery } from './queries';
  import type { JournalSearch } from './search.svelte';
  import { canSearch, hitDomId, moveActive } from './search-state';
  import SearchRangeChip from './SearchRangeChip.svelte';
  import SearchResults from './SearchResults.svelte';

  interface Props {
    search: JournalSearch;
    today: string;
    weekStart: WeekStart;
    timeFormat: TimeFormat;
    /** Opens a result: closes search, scrolls to the Entry and flashes it. `scrollTop` is where the list was. */
    onopen: (hit: Hit, scrollTop: number) => void;
    /** Esc closed search. */
    onclose: () => void;
    class?: string;
  }
  let { search, today, weekStart, timeFormat, onopen, onclose, class: cls = '' }: Props = $props();

  const LIST_ID = 'journal-search-results';

  let fieldBox = $state<HTMLElement | null>(null);
  let scroller = $state<HTMLElement | null>(null);
  let active = $state(-1);
  let restored = false;

  const filter = $derived({ query: search.committed, range: search.range });
  const enabled = $derived(canSearch(filter));
  const results = createQuery(() => ({
    ...searchQuery(filter.query.trim(), filter.range),
    enabled,
  }));
  const data = $derived(enabled ? results.data : undefined);
  const flat = $derived(data ? [...data.exact, ...data.similar] : []);
  const activeHit = $derived(flat[active]);
  const empty = $derived(data !== undefined && flat.length === 0);

  /** Focuses the field; `select` also selects its text (⌘F while search is already open). */
  export function focus(select = true) {
    const input = fieldBox?.querySelector('input');
    input?.focus();
    if (select) input?.select();
  }

  onMount(() => focus(false));

  // A new set of results starts with nothing highlighted.
  $effect(() => {
    void flat;
    active = -1;
  });

  // Back to results: put the list where it was left, once there is something to scroll.
  $effect(() => {
    if (restored || !data || !scroller || search.savedScroll === 0) return;
    restored = true;
    const top = search.savedScroll;
    void tick().then(() => scroller?.scrollTo({ top }));
  });

  // Keep the highlighted row in view as the arrow keys move it.
  $effect(() => {
    if (activeHit) {
      document.getElementById(hitDomId(LIST_ID, activeHit))?.scrollIntoView({ block: 'nearest' });
    }
  });

  function open(hit: Hit) {
    onopen(hit, scroller?.scrollTop ?? 0);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.isComposing) return;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      if (flat.length === 0) return;
      e.preventDefault();
      active = moveActive(active, e.key, flat.length);
    } else if (e.key === 'Enter') {
      const hit = flat[active] ?? flat[0];
      if (!hit) return;
      e.preventDefault();
      open(hit);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      const closing = search.query === '';
      search.escape();
      if (closing) onclose();
    }
  }
</script>

<div class="flex h-full flex-col bg-bg {cls}">
  <div class="shrink-0 px-(--pad) pt-2 pb-3">
    <div class="mx-auto flex max-w-(--journal-width) items-center gap-2">
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div bind:this={fieldBox} class="relative min-w-0 flex-1" {onkeydown}>
        <Search class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-faint" />
        <Input
          value={search.query}
          oninput={(e) => search.setQuery(e.currentTarget.value)}
          label="Search Journal"
          placeholder="Search Journal"
          role="combobox"
          aria-expanded={flat.length > 0}
          aria-controls={LIST_ID}
          aria-activedescendant={activeHit ? hitDomId(LIST_ID, activeHit) : undefined}
          autocomplete="off"
          autocapitalize="off"
          spellcheck={false}
          enterkeyhint="search"
          style="padding-inline: 2.25rem"
        />
        {#if search.query !== ''}
          <IconButton
            icon={X}
            label="Clear search"
            size="xs"
            variant="quiet"
            class="absolute top-1/2 right-2 -translate-y-1/2"
            onclick={() => {
              search.setQuery('');
              focus(false);
            }}
          />
        {/if}
      </div>
      <SearchRangeChip
        range={search.range}
        {today}
        {weekStart}
        onchange={(r) => search.setRange(r)}
        onrestorefocus={() => focus(false)}
      />
    </div>
  </div>

  <div bind:this={scroller} class="min-h-0 flex-1 overflow-y-auto px-(--pad) pb-6">
    <div class="mx-auto max-w-(--journal-width)">
      {#if !enabled}
        <EmptyState
          icon={Search}
          hue="--hue-journal"
          title="Search your Journal"
          description="Words in any Entry, or pick a date range to browse a period. Try “river” or “design”."
        />
      {:else if results.isError}
        <ErrorBanner error={results.error} onretry={() => results.refetch()} />
      {:else if !data}
        <div class="flex flex-col gap-3 py-2">
          <Skeleton class="h-5 w-40" />
          <Skeleton class="h-14 w-full" />
          <Skeleton class="h-14 w-full" />
        </div>
      {:else if empty}
        <EmptyState
          icon={SearchX}
          hue="--hue-journal"
          title={filter.query.trim() === '' ? 'No entries in this range' : `No entries match “${filter.query.trim()}”`}
          description={filter.query.trim() === ''
            ? 'Try a wider range.'
            : 'Check the spelling, or try fewer words.'}
        />
      {:else}
        <SearchResults results={data} {today} {timeFormat} {active} id={LIST_ID} onopen={open} />
      {/if}
    </div>
  </div>
</div>
