<!--
  TodoPage: Todo's single route. The view (Inbox, Today, Upcoming, All, Done or a List) is held in
  Todo's own state, set from the sidebar tree; this shows the one that is current. The open Task's
  sheet (`?task=‹id›`), the "steps still open" question and the Task keys live here too: E starts a
  Quick edit, 1 to 4 set the Priority and ⇧Space marks Won't do (or skips a repeating Task) on the row under the pointer,
  the focused row, or the open sheet's Task; they stay out of text fields, menus and Dialogs. ⌥↑ and
  ⌥↓ move that Task one place within its group (the manual order). Tasks are dragged by their rows
  (see drag.svelte.ts); the layer that floats above the page while one is held lives here.
-->
<script lang="ts">
  import { onDestroy } from 'svelte';
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
  import { quickEditing, quickEditKey } from './quick-edit.svelte';
  import { setTaskPriority, wontDoTask } from './task-actions';
  import { inSheetScope, isTyping, priorityKey, taskRows, wontKey } from './task-keys.svelte';
  import TaskSheet from './TaskSheet.svelte';
  import { taskSheet } from './task-sheet.svelte';
  import TodayPage from './TodayPage.svelte';
  import UpcomingPage from './UpcomingPage.svelte';

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

  /** The Task a Task key means, or none when the key is not for Todo right now. */
  function keyTarget(e: KeyboardEvent): Task | null {
    if (isTyping(e.target)) return null;
    const el = e.target instanceof Element ? e.target : null;
    // The sheet's own Task, unless focus is in a picker or Dialog opened over it.
    if (taskSheet.id !== null) return inSheetScope(e.target) ? taskSheet.task : null;
    if (el?.closest('[role="menu"], [role="listbox"], [role="dialog"], [role="alertdialog"]')) return null;
    return taskRows.get(el?.closest<HTMLElement>('[data-task-row]')?.dataset.taskRow) ?? taskRows.get(taskRows.hoveredId);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.defaultPrevented) return;
    if (quickEditKey(e) && taskSheet.id === null) {
      const row = keyTarget(e);
      if (row?.state === 'open') {
        e.preventDefault();
        quickEditing.start(row.id);
      }
      return;
    }
    if (e.altKey && !e.metaKey && !e.ctrlKey && !e.shiftKey && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
      const row = keyTarget(e);
      if (row?.state === 'open' && taskSheet.id === null && drag.keyboardMove(row, e.key === 'ArrowUp' ? -1 : 1)) {
        e.preventDefault();
      }
      return;
    }
    const priority = priorityKey(e);
    const wont = wontKey(e);
    if (priority === null && !wont) return;
    const task = keyTarget(e);
    if (!task || task.state !== 'open') return;
    e.preventDefault();
    if (priority !== null) void setTaskPriority(task, priority);
    else void wontDoTask(task);
  }

  // A button takes Space on key-up: keep ⇧Space from also pressing the focused diamond.
  const onkeyup = (e: KeyboardEvent) => wontKey(e) && keyTarget(e) !== null && e.preventDefault();
</script>

<svelte:window {onkeydown} {onkeyup} />

<div class="selectable h-full overflow-y-auto p-(--pad)">
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

{#if taskSheet.id !== null}
  <TaskSheet id={taskSheet.id} />
{/if}
<OpenStepsDialog />
<DragLayer kind="task" />
