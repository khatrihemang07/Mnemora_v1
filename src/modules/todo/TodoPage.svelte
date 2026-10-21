<!--
  TodoPage: Todo's single route. The view (Inbox, Today, Upcoming, All, Done or a List) is held in
  Todo's own state, set from the sidebar tree; this shows the one that is current. Search (the top
  bar's button, ⌘F, or ⌘K) lays its results over the view, which stays mounted underneath. The open
  Task's sheet (`?task=‹id›`) and the "steps still open" question live here too, and so does the
  desktop key map: every key goes through the one reducer in task-keymap.ts, which says what it
  means for the Task with the focus ring, the one under the pointer or the open sheet's Task; the
  keys stay out of text fields, menus and Dialogs. Tasks are dragged by their rows (see
  drag.svelte.ts); the layer that floats above the page while one is held lives here.
  On a phone this is also where the chrome lives: the segments pinned under the large title, the
  Lists sheet behind ☰, the swipe on the title or segment bar that moves between Today, Upcoming and
  Inbox (rows keep their own sideways swipes), the teal + button, and scrolling to the top when the
  Todo tab is tapped again.
-->
<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import Plus from '~icons/lucide/plus';
  import type { Task } from '../../lib/ipc/bindings';
  import { keyboard } from '../../lib/keyboard.svelte';
  import { viewSwipeStep } from '../../lib/swipe';
  import { addDays } from '../../lib/ui/calendar';
  import { pageHeader } from '../../lib/ui/page-header.svelte';
  import { viewport } from '../../lib/viewport.svelte';
  import { clock } from '../../lib/time/clock.svelte';
  import { createQuery } from '@tanstack/svelte-query';
  import AllPage from './AllPage.svelte';
  import { todoClock } from './clock.svelte';
  import DonePage from './DonePage.svelte';
  import DragLayer from './DragLayer.svelte';
  import { drag } from './drag.svelte';
  import InboxPage from './InboxPage.svelte';
  import ListPage from './ListPage.svelte';
  import ListsSheet from './ListsSheet.svelte';
  import { todoNav } from './nav.svelte';
  import OpenStepsDialog from './OpenStepsDialog.svelte';
  import PhoneSegments from './PhoneSegments.svelte';
  import { followCoreSettings } from './prefs.svelte';
  import { listsQuery } from './queries';
  import { quickAdd } from './quick-add.svelte';
  import { quickEditing } from './quick-edit.svelte';
  import { PALETTE_CLOSE_MS, searchLauncher, todoSearch } from './search.svelte';
  import {
    dateTaskToday,
    deleteTaskWithUndo,
    setTaskPriority,
    tickTask,
    untickTask,
    wontDoTask,
  } from './task-actions';
  import { taskFocus } from './task-focus.svelte';
  import { inSheetScope, isTyping, overlayOpen, taskRows } from './task-keys.svelte';
  import { IDLE_KEYS, type KeyAction, type KeyScene, reduceKey } from './task-keymap';
  import TaskSheet from './TaskSheet.svelte';
  import { taskSheet } from './task-sheet.svelte';
  import TodayPage from './TodayPage.svelte';
  import TodoSearch from './TodoSearch.svelte';
  import UpcomingPage from './UpcomingPage.svelte';
  import { addPlaceFor, smartView, swipedView } from './views';

  // The shared clock ticks only while someone is subscribed; the views read the day and the minute
  // from it, so midnight rolls them over without any polling of ours.
  onDestroy(clock.subscribe());
  followCoreSettings();

  const view = $derived(todoNav.view);

  // What the drag labels need: today and the Lists by id.
  const sidebar = createQuery(() => listsQuery());
  $effect(() => {
    drag.context = {
      today: todoClock.today,
      lists: new Map((sidebar.data?.lists ?? []).map((e) => [e.list.id, e.list])),
    };
  });

  // Search belongs to the view it was opened on: picking another one (or leaving Todo) ends it.
  $effect(() => {
    void todoNav.view;
    untrack(() => todoSearch.close());
  });
  onDestroy(() => {
    todoSearch.close();
    taskFocus.id = null;
  });

  // A search asked for from ⌘K (possibly just before this page mounted) opens with its words.
  let searchView = $state<ReturnType<typeof TodoSearch> | null>(null);
  $effect(() => {
    const query = searchLauncher.pending;
    if (query === null) return;
    searchLauncher.pending = null;
    untrack(() => todoSearch.open(query));
    // The palette gives focus back to where it was as it closes; take it again once it has.
    setTimeout(() => searchView?.focus(false), PALETTE_CLOSE_MS);
  });

  // --- Phone chrome ----------------------------------------------------------------------------
  let listsOpen = $state(false);
  let scroller = $state<HTMLElement | null>(null);

  // The segments sit under each page's large title, so the title row asks for them (see PageHeader).
  $effect(() => {
    if (!viewport.isPhone) return;
    return pageHeader.setPhoneBar(segments);
  });

  // The Todo tab tapped again: back to the top (Today was already chosen by `todoNav.home`).
  let seenTopRequests = todoNav.topRequests;
  $effect(() => {
    const asked = todoNav.topRequests;
    if (asked === seenTopRequests) return;
    seenTopRequests = asked;
    const reduce =
      document.documentElement.dataset.reduceMotion === 'true' ||
      window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    untrack(() => scroller?.scrollTo({ top: 0, behavior: reduce ? 'auto' : 'smooth' }));
  });

  /** Where a sideways swipe on the title or segment bar began. */
  let swipeFrom: { x: number; y: number; id: number } | null = null;
  const swipeStart = (e: PointerEvent) => {
    swipeFrom = null;
    if (e.pointerType === 'mouse' || !viewport.isPhone) return;
    if ((e.target as Element).closest('[data-view-swipe]') && !(e.target as Element).closest('[data-task-row]')) {
      swipeFrom = { x: e.clientX, y: e.clientY, id: e.pointerId };
    }
  };
  const swipeEnd = (e: PointerEvent) => {
    const from = swipeFrom;
    swipeFrom = null;
    if (!from || from.id !== e.pointerId) return;
    const step = viewSwipeStep(e.clientX - from.x, e.clientY - from.y);
    const next = step === 0 ? null : swipedView(todoNav.view, step);
    if (next && step !== 0) todoNav.swipeTo(next, step);
  };

  /** The + button: the capture box in a bottom sheet, for the place this view stands for. */
  function addHere() {
    const today = todoClock.today;
    const place = addPlaceFor(todoNav.view, today, addDays(today, 1));
    quickAdd.openFloating(place.date, place.listId);
  }

  let keys = IDLE_KEYS;

  /** The Task a key means, and how it was pointed at: the focus ring, or only the pointer. */
  function keyTarget(e: KeyboardEvent): { task: Task | null; reach: KeyScene['target'] } {
    // The sheet's own Task, unless focus is in a picker or Dialog opened over it.
    if (taskSheet.id !== null) return { task: taskSheet.task, reach: 'ring' };
    const el = e.target instanceof Element ? e.target : null;
    const fromFocus = taskRows.get(el?.closest<HTMLElement>('[data-task-row]')?.dataset.taskRow);
    if (fromFocus) return { task: fromFocus, reach: 'ring' };
    // The ring counts while focus is on the page itself, not in the sidebar or a field.
    if (!el || el === document.body || el.closest('[data-task-scope]')) {
      const ringed = taskRows.get(taskFocus.id);
      if (ringed) return { task: ringed, reach: 'ring' };
    }
    const hovered = taskRows.get(taskRows.hoveredId);
    return { task: hovered, reach: hovered ? 'hover' : 'none' };
  }

  function sceneFor(e: KeyboardEvent): { scene: KeyScene; task: Task | null } {
    const sheetOpen = taskSheet.id !== null;
    const { task, reach } = keyTarget(e);
    const el = e.target instanceof Element ? e.target : null;
    return {
      task,
      scene: {
        typing: isTyping(e.target),
        overlay: sheetOpen ? !inSheetScope(e.target) || overlayOpen(true) : overlayOpen(false),
        sheet: sheetOpen,
        target: reach,
        taskOpen: task?.state === 'open',
        onControl: el?.closest('button, a, [role="checkbox"], summary') != null,
      },
    };
  }

  function onkeydown(e: KeyboardEvent) {
    const { scene, task } = sceneFor(e);
    const result = reduceKey(keys, e, scene, Date.now());
    keys = result.state;
    if (!result.action) return;
    e.preventDefault();
    perform(result.action, task);
  }

  /** Runs what the key map decided. The sheet's own keys (⌘↑ ⌘↓ ⌘↵) are handled by the sheet. */
  function perform(action: KeyAction, task: Task | null) {
    switch (action.type) {
      case 'ring':
        taskFocus.move(action.dir);
        return;
      case 'clear-ring':
        taskFocus.clear();
        if (todoSearch.active) searchView?.focus(false);
        return;
      case 'search':
        if (todoSearch.active) searchView?.focus();
        else todoSearch.open();
        return;
      case 'go':
        todoSearch.close();
        todoNav.go(smartView(action.to));
        return;
      case 'sheet-step':
      case 'sheet-close':
        return;
    }
    if (!task) return;
    const next = taskFocus.id === task.id ? taskFocus.neighbour(task.id) : null;
    const settle = () => taskFocus.id === task.id && taskFocus.keepAfter(task.id, next);
    switch (action.type) {
      case 'complete':
        void (task.state === 'open' ? tickTask(task) : untickTask(task)).then(settle);
        break;
      case 'wont':
        void wontDoTask(task).then(settle);
        break;
      case 'open':
        taskSheet.open(task.id);
        break;
      case 'quick-edit':
        quickEditing.start(task.id);
        break;
      case 'when':
        taskRows.pickers(task.id)?.when();
        break;
      case 'today':
        void dateTaskToday(task).then(settle);
        break;
      case 'move-list':
        taskRows.pickers(task.id)?.moveToList();
        break;
      case 'priority':
        void setTaskPriority(task, action.value);
        break;
      case 'reorder':
        drag.keyboardMove(task, action.dir);
        break;
      case 'delete':
        void deleteTaskWithUndo(task).then((ok) => ok && settle());
        break;
    }
  }

  // A button takes Space on key-up: keep ⇧Space from also pressing the focused diamond.
  const onkeyup = (e: KeyboardEvent) =>
    e.key === ' ' && e.shiftKey && !isTyping(e.target) && keyTarget(e).task !== null && e.preventDefault();

  // The ring belongs to the page: focus moving to a field, the sidebar or a Dialog, or a click
  // anywhere but a row, drops it.
  const onfocusin = (e: FocusEvent) => {
    const el = e.target instanceof Element ? e.target : null;
    if (el && el !== document.body && !el.closest('[data-task-row], [data-task-scope]')) {
      taskFocus.id = null;
    }
  };
  const onpointerdown = (e: PointerEvent) => {
    if (!(e.target as Element | null)?.closest?.('[data-task-row]')) taskFocus.id = null;
  };
</script>

<svelte:window {onkeydown} {onkeyup} {onfocusin} {onpointerdown} />

{#snippet segments()}
  <PhoneSegments onmore={() => (listsOpen = true)} moreOpen={listsOpen} />
{/snippet}

<div class="relative h-full">
  <div
    bind:this={scroller}
    data-task-scope="view"
    class="selectable h-full overflow-y-auto p-(--pad) phone:pb-24"
    inert={todoSearch.active}
    aria-hidden={todoSearch.active}
    onpointerdown={swipeStart}
    onpointerup={swipeEnd}
    onpointercancel={() => (swipeFrom = null)}
  >
    {#key `${view.kind}:${view.id}`}
      <div class={todoNav.slide === 'next' ? 'view-slide-next' : todoNav.slide === 'prev' ? 'view-slide-prev' : ''}>
        {#if view.kind === 'list'}
          <ListPage id={view.id} />
        {:else if view.id === 'inbox'}
          <InboxPage />
        {:else if view.id === 'all'}
          <AllPage />
        {:else if view.id === 'today'}
          <TodayPage />
        {:else if view.id === 'upcoming'}
          <UpcomingPage />
        {:else}
          <DonePage />
        {/if}
      </div>
    {/key}
  </div>
  {#if todoSearch.active}
    <div class="absolute inset-0 z-10">
      <TodoSearch bind:this={searchView} />
    </div>
  {:else if viewport.isPhone && !keyboard.open}
    <button
      type="button"
      aria-label="New task"
      onclick={addHere}
      class="absolute right-4 bottom-4 z-20 grid size-14 cursor-pointer place-items-center rounded-lg bg-hue-todo text-on-accent
        shadow-[0_8px_22px_-6px_color-mix(in_oklch,var(--hue-todo)_60%,transparent)] transition-transform duration-(--dur) ease-out active:scale-[0.94]"
    >
      <Plus class="size-6" />
    </button>
  {/if}
</div>

<ListsSheet bind:open={listsOpen} />

{#if taskSheet.id !== null}
  <TaskSheet id={taskSheet.id} />
{/if}
<OpenStepsDialog />
<DragLayer kind="task" />
