<!--
  TodayPage: what is due today. Headed with the full date, a thin progress bar and "N of M done".
  A red Overdue group sits on top with "Move all to Today" and "Move all to…" (Undo); then today's
  Tasks (timed ones first, by time, then the rest in manual order); a Task due today whose time has
  passed stays here with its time in red. Tasks done today sit struck through under "Completed
  today". "+ New task" adds a Task due today. Midnight rolls the page over by itself: the query is
  keyed by the day.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import { todoClock } from './clock.svelte';
  import { longDate } from './dates';
  import GroupHeader from './GroupHeader.svelte';
  import NewTaskRow from './NewTaskRow.svelte';
  import { listsQuery, todayQuery } from './queries';
  import { moveAllOverdue, tickTask, untickTask } from './task-actions';
  import TaskRow from './TaskRow.svelte';
  import { VIEW_ICONS } from './view-icons';
  import WhenHost from './WhenHost.svelte';

  const today = createQuery(() => todayQuery(todoClock.today));
  const sidebar = createQuery(() => listsQuery());
  const lists = $derived(new Map((sidebar.data?.lists ?? []).map((e) => [e.list.id, e.list])));

  const view = $derived(today.data);
  const done = $derived(view?.done.length ?? 0);
  const percent = $derived(view && view.total > 0 ? Math.round((done / view.total) * 100) : 0);

  let moveOpen = $state(false);
  let moveButton = $state<HTMLButtonElement | null>(null);

  const link =
    'cursor-pointer rounded-sm px-1.5 py-0.5 text-xs font-semibold text-hue-todo transition-colors duration-(--dur) ease-out hover:bg-hover';
</script>

<PageHeader title="Today" icon={VIEW_ICONS.today} hue="--hue-todo" />
<div class="mx-auto max-w-3xl">
  {#if today.isPending}
    <div class="flex flex-col gap-3 py-2">
      <Skeleton class="h-5 w-64" />
      <Skeleton class="h-5 w-48" />
    </div>
  {:else if today.isError}
    <ErrorBanner message="Today couldn't load." error={today.error} onretry={() => today.refetch()} />
  {:else if view}
    <div class="mb-1 flex flex-wrap items-baseline gap-x-3 gap-y-1 px-2">
      <!-- On a phone the page header already shows the large "Today". -->
      <h1
        class="font-display text-[1.7rem] leading-[1.15] phone:hidden"
        style="font-weight: var(--display-weight); letter-spacing: var(--display-track)"
      >
        Today
      </h1>
      <p class="text-sm text-faint">{longDate(todoClock.today)}</p>
      {#if view.total > 0}
        <div class="ml-auto flex items-center gap-2 self-center text-xs text-muted">
          <div
            class="h-1.5 w-24 overflow-hidden rounded-full bg-line"
            role="progressbar"
            aria-label="Done today"
            aria-valuemin={0}
            aria-valuemax={view.total}
            aria-valuenow={done}
          >
            <div class="h-full rounded-full bg-hue-todo transition-[width] duration-(--dur) ease-out" style="width: {percent}%"></div>
          </div>
          <span class="tabular-nums">{done} of {view.total} done</span>
        </div>
      {/if}
    </div>

    {#if view.overdue.length > 0}
      <section aria-label="Overdue">
        <GroupHeader label="Overdue" count={view.overdue.length} tone="danger">
          {#snippet actions()}
            <button type="button" bind:this={moveButton} class={link} onclick={() => (moveOpen = true)}>
              Move all to…
            </button>
            <button type="button" class={link} onclick={() => void moveAllOverdue(todoClock.today)}>
              Move all to Today
            </button>
          {/snippet}
        </GroupHeader>
        {#each view.overdue as task (task.id)}
          <TaskRow {task} ontick={tickTask} list={task.list_id ? (lists.get(task.list_id) ?? null) : null} />
        {/each}
      </section>
    {/if}

    <section aria-label="Today">
      {#if view.overdue.length > 0}
        <GroupHeader label="Today" count={view.tasks.length} />
      {/if}
      {#each view.tasks as task (task.id)}
        <TaskRow {task} ontick={tickTask} hideDate list={task.list_id ? (lists.get(task.list_id) ?? null) : null} />
      {/each}
      {#if view.overdue.length === 0 && view.tasks.length === 0}
        <p class="px-2 pt-2 pb-1 text-sm text-faint">
          Nothing due today.
          {done > 0 ? `You finished ${done} today.` : 'Enjoy the space, or plan something.'}
        </p>
      {/if}
      <NewTaskRow date={todoClock.today} />
    </section>

    {#if view.done.length > 0}
      <section aria-label="Completed today">
        <GroupHeader label="Completed today" count={view.done.length} />
        {#each view.done as task (task.id)}
          <TaskRow {task} ontick={tickTask} onuntick={untickTask} list={task.list_id ? (lists.get(task.list_id) ?? null) : null} />
        {/each}
      </section>
    {/if}
  {/if}
</div>

<WhenHost
  bind:open={moveOpen}
  anchor={moveButton}
  title="Move all to…"
  dateOnly
  onchange={(next) => next.date && void moveAllOverdue(next.date)}
/>
