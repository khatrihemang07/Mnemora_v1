<!--
  DonePage: what has been done, under a serif heading per day (Today, Yesterday, then the weekday
  with a small date), newest day first and newest first within a day. Rows show the time it was
  done and its List; the diamond reopens it (an old occurrence of a repeating Task explains why it
  cannot). Tasks marked Won't do are listed with the done ones, with a ⊗ and "Won't do" and the time.
  Days load a page at a time as you scroll: a sentinel under the list asks for the next page just
  before it comes into view.
-->
<script lang="ts">
  import { createInfiniteQuery, createQuery } from '@tanstack/svelte-query';
  import DayHeading from '../../lib/ui/DayHeading.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import TodoActions from './TodoActions.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import { todoClock } from './clock.svelte';
  import { doneHeading } from './dates';
  import { doneQuery, listsQuery } from './queries';
  import { tickTask, untickTask } from './task-actions';
  import TaskRow from './TaskRow.svelte';
  import { VIEW_ICONS } from './view-icons';

  const done = createInfiniteQuery(() => doneQuery(todoClock.today));
  const sidebar = createQuery(() => listsQuery());
  const lists = $derived(new Map((sidebar.data?.lists ?? []).map((e) => [e.list.id, e.list])));
  const listOf = (id: string | null) => (id ? (lists.get(id) ?? null) : null);

  const days = $derived(done.data?.pages.flatMap((p) => p.days) ?? []);

  let sentinel = $state<HTMLElement | null>(null);
  // Ask for the next page a screen before the end shows. The page has no scroll position of its
  // own to track: the observer sees the sentinel come near whatever scrolls it.
  $effect(() => {
    if (!sentinel) return;
    const watch = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting) && done.hasNextPage && !done.isFetchingNextPage) {
          void done.fetchNextPage();
        }
      },
      { rootMargin: '0px 0px 400px 0px' },
    );
    watch.observe(sentinel);
    return () => watch.disconnect();
  });
</script>

<PageHeader title="Done" icon={VIEW_ICONS.done} hue="--hue-todo">
  {#snippet actions()}<TodoActions />{/snippet}
</PageHeader>
<div class="mx-auto max-w-3xl">
  {#if done.isPending}
    <div class="flex flex-col gap-3 py-2">
      <Skeleton class="h-5 w-64" />
      <Skeleton class="h-5 w-48" />
    </div>
  {:else if done.isError}
    <ErrorBanner message="Done couldn't load." error={done.error} onretry={() => done.refetch()} />
  {:else if days.length === 0}
    <EmptyState
      icon={VIEW_ICONS.done}
      hue="--hue-todo"
      title="Nothing done yet"
      description="Tasks you finish are listed here by day."
    />
  {:else}
    {#each days as day (day.date)}
      {@const heading = doneHeading(day.date, todoClock.today)}
      <section aria-label={heading.name}>
        <DayHeading name={heading.name} small={heading.small} count={day.tasks.length} first />
        {#each day.tasks as task (task.id)}
          <TaskRow {task} ontick={tickTask} onuntick={untickTask} list={listOf(task.list_id)} />
        {/each}
      </section>
    {/each}
    <div bind:this={sentinel} aria-hidden="true" class="h-px"></div>
    {#if done.isFetchingNextPage}
      <div class="flex flex-col gap-3 px-2 py-3">
        <Skeleton class="h-5 w-56" />
        <Skeleton class="h-5 w-40" />
      </div>
    {:else if !done.hasNextPage}
      <p class="px-2 pt-4 pb-2 text-center text-xs text-faint">That is everything.</p>
    {/if}
  {/if}
</div>
