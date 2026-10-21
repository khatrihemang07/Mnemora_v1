<!--
  TodoPage: Todo's single route. The view (Inbox, Today, Upcoming, All, Done or a List) is held in
  Todo's own state, set from the sidebar tree; this shows the one that is current. Search (the top
  bar's button, ⌘F, or ⌘K) lays its results over the view, which stays mounted underneath. The open
  Task's sheet (`?task=‹id›`) and the "steps still open" question live here too, and so does the
  desktop key map: every key goes through the one reducer in task-keymap.ts, which says what it
  means for the Task with the focus ring, the one under the pointer or the open sheet's Task; the
  keys stay out of text fields, menus and Dialogs. Tasks are dragged by their rows (see
  drag.svelte.ts); the layer that floats above the page while one is held lives here.
-->
<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import type { Task } from '../../lib/ipc/bindings';
  import { clock } from '../../lib/time/clock.svelte';
  import { createQuery } from '@tanstack/svelte-query';
  import AllPage from './AllPage.svelte';
  import { todoClock } from './clock.svelte';
  import DonePage from './DonePage.svelte';
  import DragLayer from './DragLayer.svelte';
  import { drag } from './drag.svelte';
  import InboxPage from './InboxPage.svelte';
  import ListPage from './ListPage.svelte';
  import { todoNav } from './nav.svelte';
  import OpenStepsDialog from './OpenStepsDialog.svelte';
  import { followCoreSettings } from './prefs.svelte';
  import { listsQuery } from './queries';
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
  import { smartView } from './views';

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

<div class="relative h-full">
  <div
    data-task-scope="view"
    class="selectable h-full overflow-y-auto p-(--pad)"
    inert={todoSearch.active}
    aria-hidden={todoSearch.active}
  >
    {#if view.kind === 'list'}
      {#key view.id}<ListPage id={view.id} />{/key}
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
  {#if todoSearch.active}
    <div class="absolute inset-0 z-10">
      <TodoSearch bind:this={searchView} />
    </div>
  {/if}
</div>

{#if taskSheet.id !== null}
  <TaskSheet id={taskSheet.id} />
{/if}
<OpenStepsDialog />
<DragLayer kind="task" />
