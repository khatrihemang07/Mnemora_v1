<!--
  TodoSearch: Todo's search, laid over the current view (which stays mounted underneath). A field
  with a search icon and a clear ×, then live results 150ms after you stop typing: the open Tasks
  first, a Done group after them (Won't do included), then "Similar matches" for typos. Each row is
  a normal Task row with the matched words tinted and, under the title, the note or Comment line
  that matched. Opening a Task shows its sheet over the results; Esc on the sheet returns here with
  the words intact. ↓ and ↑ walk the results from the field (and ↑ from the first returns to it),
  Enter opens the first or the focused one, Esc clears the words and a second Esc closes search.
  On a phone the page's own header is covered, so a Cancel button closes search, and so does Back.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { onMount } from 'svelte';
  import Search from '~icons/lucide/search';
  import SearchX from '~icons/lucide/search-x';
  import X from '~icons/lucide/x';
  import type { TaskHit } from '../../lib/ipc/bindings';
  import { interceptBack } from '../../lib/router';
  import { viewport } from '../../lib/viewport.svelte';
  import Button from '../../lib/ui/Button.svelte';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import Input from '../../lib/ui/Input.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import GroupHeader from './GroupHeader.svelte';
  import { searchQuery } from './queries';
  import { canSearch, litWords, resultsLabel, spansText, splitHits } from './search-results';
  import { todoSearch } from './search.svelte';
  import { useSidebar } from './sidebar.svelte';
  import { taskFocus } from './task-focus.svelte';
  import { taskSheet } from './task-sheet.svelte';
  import { tickTask, untickTask } from './task-actions';
  import TaskRow from './TaskRow.svelte';

  const sidebar = useSidebar();
  const lists = $derived(sidebar.lists);

  const query = $derived(todoSearch.committed.trim());
  const enabled = $derived(canSearch(query));
  const results = createQuery(() => ({ ...searchQuery(query), enabled }));
  const data = $derived(enabled ? results.data : undefined);
  const exact = $derived(splitHits(data?.exact ?? []));
  const empty = $derived(data !== undefined && data.exact.length + data.similar.length === 0);
  const total = $derived((data?.exact.length ?? 0) + (data?.similar.length ?? 0));

  let fieldBox = $state<HTMLElement | null>(null);

  /** Focuses the field; `select` also selects its text (⌘F while search is already open). */
  export function focus(select = true) {
    const input = fieldBox?.querySelector('input');
    input?.focus();
    if (select) input?.select();
  }

  onMount(() => {
    focus(false);
    // Android Back leaves search before it leaves the page.
    return interceptBack(() => todoSearch.close());
  });

  /** ↓ from the field enters the results; ↑ from the first leaves them for the field. */
  function onkeydown(e: KeyboardEvent) {
    if (e.isComposing) return;
    const ids = taskFocus.ids();
    if (e.key === 'ArrowDown' && ids.length > 0) {
      e.preventDefault();
      taskFocus.focus(ids[0] as string);
    } else if (e.key === 'Enter') {
      const first = ids[0];
      if (first === undefined) return;
      e.preventDefault();
      taskSheet.open(first);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      todoSearch.escape();
    }
  }

  /** ↑ on the first result goes back to the field (the other keys are the key map's). */
  function onresultkeys(e: KeyboardEvent) {
    if (e.key !== 'ArrowUp' || e.altKey || e.metaKey || e.ctrlKey || e.shiftKey) return;
    const ids = taskFocus.ids();
    if (ids[0] !== undefined && taskFocus.id === ids[0]) {
      e.preventDefault();
      e.stopPropagation();
      taskFocus.clear();
      focus(false);
    }
  }

  const rowProps = (hit: TaskHit) => ({
    task: hit.task,
    lit: litWords(hit.title),
    snippet:
      hit.snippet_source !== null && hit.snippet.length > 0
        ? { spans: hit.snippet, source: hit.snippet_source }
        : undefined,
    list: hit.task.list_id ? (lists.get(hit.task.list_id) ?? null) : null,
    ontick: tickTask,
    onuntick: untickTask,
  });
</script>

{#snippet rows(hits: TaskHit[])}
  {#each hits as hit (hit.task.id)}
    <TaskRow {...rowProps(hit)} />
  {/each}
{/snippet}

<div class="flex h-full flex-col bg-bg">
  <div class="shrink-0 px-(--pad) pt-(--pad) pb-2">
    <div class="mx-auto flex max-w-3xl items-center gap-2">
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div bind:this={fieldBox} class="relative min-w-0 flex-1" {onkeydown} data-todo-search>
        <Search class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-faint" />
        <Input
          value={todoSearch.query}
          oninput={(e) => todoSearch.setQuery(e.currentTarget.value)}
          label="Search tasks"
          placeholder="Search tasks, notes and comments"
          autocomplete="off"
          autocapitalize="off"
          spellcheck={false}
          enterkeyhint="search"
          style="padding-inline: 2.25rem"
        />
        {#if todoSearch.query !== ''}
          <IconButton
            icon={X}
            label="Clear search"
            size="xs"
            variant="quiet"
            class="absolute top-1/2 right-2 -translate-y-1/2"
            onclick={() => {
              todoSearch.setQuery('');
              focus(false);
            }}
          />
        {/if}
      </div>
      {#if viewport.isPhone}
        <Button variant="ghost" onclick={() => todoSearch.close()}>Cancel</Button>
      {/if}
    </div>
  </div>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    data-task-scope="results"
    class="selectable min-h-0 flex-1 overflow-y-auto px-(--pad) pb-6"
    onkeydowncapture={onresultkeys}
  >
    <div class="mx-auto max-w-3xl">
      {#if !enabled}
        <EmptyState
          icon={Search}
          hue="--hue-todo"
          title="Search your Tasks"
          description="Titles, notes and comments, done Tasks included. Try “passport” or “call mum”."
        />
      {:else if results.isError}
        <ErrorBanner message="Search couldn't run." error={results.error} onretry={() => results.refetch()} />
      {:else if !data}
        <div class="flex flex-col gap-3 py-2">
          <Skeleton class="h-5 w-40" />
          <Skeleton class="h-10 w-full" />
          <Skeleton class="h-10 w-full" />
        </div>
      {:else if empty}
        <EmptyState
          icon={SearchX}
          hue="--hue-todo"
          title="No Tasks match “{query}”"
          description="Check the spelling, or try fewer words."
        />
      {:else}
        <p class="px-2 pt-1 text-xs text-faint" role="status">
          {resultsLabel(total)}{data.truncated || data.similar_truncated
            ? ' · showing the first ones, narrow the search to see more'
            : ''}
        </p>
        {#if exact.open.length > 0}
          <section aria-label="Open">
            <GroupHeader label="Open" count={exact.open.length} />
            {@render rows(exact.open)}
          </section>
        {/if}
        {#if exact.done.length > 0}
          <section aria-label="Done">
            <GroupHeader label="Done" count={exact.done.length} />
            {@render rows(exact.done)}
          </section>
        {/if}
        {#if data.similar.length > 0}
          <section aria-label="Similar matches">
            <div class="mt-6 mb-1 flex items-center gap-2.5 px-2 text-2xs font-semibold tracking-[0.08em] text-faint uppercase after:h-px after:flex-1 after:bg-line">
              Similar matches
            </div>
            <p class="mb-2 px-2 text-xs text-faint">Close to what you typed, with a typo or two.</p>
            {@render rows(data.similar)}
          </section>
        {/if}
      {/if}
    </div>
  </div>
</div>
