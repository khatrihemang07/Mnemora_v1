<!--
  UpcomingPage: dated Tasks from tomorrow on, only days that have Tasks, each under a serif day
  header with a small date, Overdue on top (with Move all, like Today). A sticky week strip above
  shows a dot on days with Tasks; clicking a day scrolls to it, and an empty day appears with an add
  row while it is selected (picking it again lets it go). "Add to this day" creates a Task due that
  day. Midnight rolls the page over by itself.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { tick } from 'svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import { todoClock } from './clock.svelte';
  import { dayHeading } from './dates';
  import GroupHeader from './GroupHeader.svelte';
  import NewTaskRow from './NewTaskRow.svelte';
  import { todoPrefs } from './prefs.svelte';
  import { listsQuery, upcomingQuery } from './queries';
  import { moveAllOverdue, tickTask } from './task-actions';
  import TaskRow from './TaskRow.svelte';
  import { VIEW_ICONS } from './view-icons';
  import WeekStrip from './WeekStrip.svelte';
  import WhenHost from './WhenHost.svelte';

  const upcoming = createQuery(() => upcomingQuery(todoClock.today));
  const sidebar = createQuery(() => listsQuery);
  const lists = $derived(new Map((sidebar.data?.lists ?? []).map((e) => [e.list.id, e.list])));
  const listOf = (id: string | null) => (id ? (lists.get(id) ?? null) : null);

  const view = $derived(upcoming.data);
  const busy = $derived(new Set(view?.days.map((d) => d.date)));

  /** The day picked in the strip: shown (empty or not) and scrolled to. */
  let selected = $state<string | null>(null);
  // Midnight can make the picked day today or past.
  $effect(() => {
    if (selected !== null && selected <= todoClock.today) selected = null;
  });

  /** Days to show: those with Tasks, plus the picked day when it has none. */
  const days = $derived.by(() => {
    const shown = (view?.days ?? []).map((d) => ({ date: d.date, tasks: d.tasks }));
    if (selected !== null && !shown.some((d) => d.date === selected)) {
      shown.push({ date: selected, tasks: [] });
      shown.sort((a, b) => (a.date < b.date ? -1 : 1));
    }
    return shown;
  });

  async function pick(date: string) {
    selected = selected === date ? null : date;
    if (selected === null) return;
    await tick();
    const reduce =
      document.documentElement.dataset.reduceMotion === 'true' ||
      window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    document
      .getElementById(`upcoming-${date}`)
      ?.scrollIntoView({ block: 'start', behavior: reduce ? 'auto' : 'smooth' });
  }

  let moveOpen = $state(false);
  let moveButton = $state<HTMLButtonElement | null>(null);

  const link =
    'cursor-pointer rounded-sm px-1.5 py-0.5 text-xs font-semibold text-hue-todo transition-colors duration-(--dur) ease-out hover:bg-hover';
</script>

<PageHeader title="Upcoming" icon={VIEW_ICONS.upcoming} hue="--hue-todo" />
<div class="mx-auto max-w-3xl">
  <div class="sticky -top-(--pad) z-10 -mt-(--pad) mb-2 bg-bg pt-(--pad)">
    <WeekStrip
      today={todoClock.today}
      weekStart={todoPrefs.weekStart}
      {busy}
      {selected}
      onpick={(date) => void pick(date)}
    />
  </div>

  {#if upcoming.isPending}
    <div class="flex flex-col gap-3 py-2">
      <Skeleton class="h-5 w-64" />
      <Skeleton class="h-5 w-48" />
    </div>
  {:else if upcoming.isError}
    <ErrorBanner message="Upcoming couldn't load." error={upcoming.error} onretry={() => upcoming.refetch()} />
  {:else if view}
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
          <TaskRow {task} ontick={tickTask} list={listOf(task.list_id)} />
        {/each}
      </section>
    {/if}

    {#if days.length === 0 && view.overdue.length === 0}
      <p class="px-2 pt-1 text-sm text-faint">
        Nothing coming up. Tasks with a date after today appear here; pick a day above to plan into it.
      </p>
    {/if}

    {#each days as day (day.date)}
      {@const heading = dayHeading(day.date, todoClock.today)}
      <section id="upcoming-{day.date}" aria-label={heading.name} class="scroll-mt-20">
        <div class="mt-5.5 mb-1 flex items-baseline gap-2 border-b border-line px-2 pb-1.5">
          <h2 class="font-display text-lg" style="font-weight: var(--display-weight)">{heading.name}</h2>
          <small class="text-xs text-faint">{heading.small}</small>
        </div>
        {#each day.tasks as task (task.id)}
          <TaskRow {task} ontick={tickTask} hideDate list={listOf(task.list_id)} />
        {/each}
        <NewTaskRow date={day.date} label="Add to this day" />
      </section>
    {/each}
  {/if}
</div>

<WhenHost
  bind:open={moveOpen}
  anchor={moveButton}
  title="Move all to…"
  dateOnly
  onchange={(next) => next.date && void moveAllOverdue(next.date)}
/>
