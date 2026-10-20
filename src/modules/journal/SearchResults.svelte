<!--
  SearchResults: what a search found, in place of the feed. Exact matches grouped by Page (small
  uppercase date header, then each Entry's time and a snippet with the matches tinted), then a
  "Similar matches" group of near-misses grouped the same way. Rows are options of one listbox: the
  search field drives `active` with the arrow keys (aria-activedescendant); a click or Enter opens.
-->
<script lang="ts">
  import type { Hit, SearchResults } from '../../lib/ipc/bindings';
  import type { TimeFormat } from '../../lib/time/format';
  import { writtenTime } from './dates';
  import { entriesLabel, groupHits, hitDomId, snippetText } from './search-state';

  interface Props {
    results: SearchResults;
    today: string;
    timeFormat: TimeFormat;
    /** Index in `[...exact, ...similar]` of the highlighted row, or -1. */
    active: number;
    onopen: (hit: Hit) => void;
    id: string;
  }
  let { results, today, timeFormat, active, onopen, id }: Props = $props();

  const exactGroups = $derived(groupHits(results.exact, today));
  const similarGroups = $derived(groupHits(results.similar, today, results.exact.length));
</script>

{#snippet group(date: string, label: string, hits: Hit[], offset: number)}
  <div role="group" aria-labelledby="{id}-day-{offset}" class="mb-4">
    <h5 id="{id}-day-{offset}" class="mb-1 text-xs font-semibold tracking-[0.06em] text-muted uppercase">{label}</h5>
    {#each hits as hit, i (hit.id)}
      <button
        type="button"
        role="option"
        id={hitDomId(id, hit)}
        aria-selected={active === offset + i}
        aria-label="{writtenTime(hit.written_at, hit.written_offset_min, timeFormat)}, {snippetText(hit.snippet)}"
        class="block w-full cursor-pointer rounded-sm px-2.5 py-2 text-left text-[0.84375rem] leading-normal transition-colors duration-(--dur) ease-out hover:bg-hover
          {active === offset + i ? 'bg-hover' : ''}"
        data-date={date}
        onclick={() => onopen(hit)}
      >
        <small class="block text-[0.71875rem] text-faint">
          {writtenTime(hit.written_at, hit.written_offset_min, timeFormat)}
        </small>
        <span class="text-ink">
          {#each hit.snippet as span, k (k)}
            {#if span.matched}<mark
                class="rounded-[3px] bg-[color-mix(in_oklch,var(--hue-journal)_28%,transparent)] px-px text-inherit"
                >{span.text}</mark
              >{:else}{span.text}{/if}
          {/each}
        </span>
      </button>
    {/each}
  </div>
{/snippet}

<!-- A listbox holds only options and groups, so the notes sit outside it or are group labels. -->
{#if results.exact.length > 0}
  <p id="{id}-summary" class="mb-2 text-xs text-faint">
    {entriesLabel(results.exact.length)} · newest first{results.truncated
      ? ' · showing the newest only, narrow the search to see more'
      : ''}
  </p>
{:else if results.similar.length > 0}
  <p id="{id}-summary" class="mb-2 text-xs text-faint">No exact matches</p>
{/if}

<div role="listbox" {id} aria-label="Search results" aria-describedby="{id}-summary">
  {#each exactGroups as g (g.date)}
    {@render group(g.date, g.label, g.hits, g.start)}
  {/each}

  {#if results.similar.length > 0}
    <div role="group" aria-labelledby="{id}-similar" aria-describedby="{id}-similar-note">
      <div
        id="{id}-similar"
        class="mt-6 mb-1 flex items-center gap-2.5 text-[11px] font-semibold tracking-[0.08em] text-faint uppercase after:h-px after:flex-1 after:bg-line"
      >
        Similar matches
      </div>
      <p id="{id}-similar-note" class="mb-3 text-xs text-faint">
        Close to what you typed, with a typo or two{results.similar_truncated
          ? ' · showing the newest only, narrow the search to see more'
          : ''}.
      </p>
    </div>
    {#each similarGroups as g (g.date)}
      {@render group(g.date, g.label, g.hits, g.start)}
    {/each}
  {/if}
</div>
