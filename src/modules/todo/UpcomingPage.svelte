<!--
  UpcomingPage: dated Tasks from tomorrow on, only days that have Tasks, each under a serif day
  header with a small date, Overdue on top (with Move all, like Today). "Group:" at the right
  regroups them by Priority or List instead (remembered); the week strip shows only grouped by Day.
  A sticky week strip above shows a dot on days with Tasks; clicking a day scrolls to it, and an
  empty day appears with an add row while it is selected (picking it again lets it go). "Add to this
  day" creates a Task due that day, and dragging a Task onto a day sets its date. Midnight rolls the
  page over by itself.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { tick } from 'svelte';
  import type { TaskGroup } from '../../lib/ipc/bindings';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import TodoActions from './TodoActions.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import { GROUP_OPTIONS, GROUP_PREF_KEYS } from './arrange';
  import { todoClock } from './clock.svelte';
  import { todoPrefs } from './prefs.svelte';
  import { savePreference, upcomingQuery } from './queries';
  import { useSidebar } from './sidebar.svelte';
  import { moveAllOverdue } from './task-actions';
  import TaskGroups from './TaskGroups.svelte';
  import ViewOptions from './ViewOptions.svelte';
  import { VIEW_ICONS } from './view-icons';
  import WeekStrip from './WeekStrip.svelte';
  import WhenHost from './WhenHost.svelte';

  const upcoming = createQuery(() => upcomingQuery(todoClock.today));
  const sidebar = useSidebar();

  const view = $derived(upcoming.data);
  const byDay = $derived(view?.group_by === 'day');
  const busy = $derived(
    new Set(view?.groups.flatMap((g) => (g.key.kind === 'day' ? [g.key.date] : []))),
  );

  /** The day picked in the strip: shown (empty or not) and scrolled to. */
  let selected = $state<string | null>(null);
  // Midnight can make the picked day today or past.
  $effect(() => {
    if (selected !== null && selected <= todoClock.today) selected = null;
  });

  /** The groups to show: Rust's, plus the picked day when it has no Tasks. */
  const groups = $derived.by((): TaskGroup[] => {
    const shown = [...(view?.groups ?? [])];
    if (byDay && selected !== null && !busy.has(selected)) {
      const key = { kind: 'day', date: selected } as const;
      const empty: TaskGroup = { key, adopt: [key], tasks: [] };
      const at = shown.findIndex((g) => g.key.kind === 'day' && g.key.date > selected!);
      shown.splice(at === -1 ? shown.length : at, 0, empty);
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

<PageHeader title="Upcoming" icon={VIEW_ICONS.upcoming} hue="--hue-todo">
  {#snippet actions()}<TodoActions />{/snippet}
</PageHeader>
<div class="mx-auto max-w-3xl">
  <div class="sticky -top-(--pad) z-10 -mt-(--pad) mb-2 bg-bg pt-(--pad) phone:top-14 phone:mt-0 phone:pt-0">
    {#if byDay || view === undefined}
      <WeekStrip
        today={todoClock.today}
        weekStart={todoPrefs.weekStart}
        {busy}
        {selected}
        onpick={(date) => void pick(date)}
      />
    {/if}
    {#if view}
      <ViewOptions
        class="justify-end {byDay ? 'mt-1' : ''}"
        groupBy={view.group_by}
        groupOptions={GROUP_OPTIONS.upcoming}
        ongroup={(next) => void savePreference(GROUP_PREF_KEYS.upcoming, next)}
      />
    {/if}
  </div>

  {#if upcoming.isPending}
    <div class="flex flex-col gap-3 py-2">
      <Skeleton class="h-5 w-64" />
      <Skeleton class="h-5 w-48" />
    </div>
  {:else if upcoming.isError}
    <ErrorBanner message="Upcoming couldn't load." error={upcoming.error} onretry={() => upcoming.refetch()} />
  {:else if view}
    {#if groups.length === 0}
      <p class="px-2 pt-1 text-sm text-faint">
        Nothing coming up. Tasks with a date after today appear here{byDay ? '; pick a day above to plan into it' : ''}.
      </p>
    {/if}

    <TaskGroups
      {groups}
      today={todoClock.today}
      lists={sidebar.lists}
      showList
      hideDate={(g) => g.key.kind === 'day'}
      addLabel={(g) => (g.key.kind === 'day' ? 'Add to this day' : 'New task')}
    >
      {#snippet overdueActions()}
        <button type="button" bind:this={moveButton} class={link} onclick={() => (moveOpen = true)}>
          Move all to…
        </button>
        <button type="button" class={link} onclick={() => void moveAllOverdue(todoClock.today)}>
          Move all to Today
        </button>
      {/snippet}
    </TaskGroups>
  {/if}
</div>

<WhenHost
  bind:open={moveOpen}
  anchor={moveButton}
  title="Move all to…"
  dateOnly
  onchange={(next) => next.date && void moveAllOverdue(next.date)}
/>
