<!--
  TaskRow: one Task in a list: the diamond to tick it (its ring shows the Priority), then its title
  (inline Markdown, wrapping, never cut) and, under it, the first line of its notes, then a meta line
  with its date and time (Today teal, Overdue red; a time already behind us red), its Repeat, the
  Checklist progress "1/3" and the number of comments. Clicking the title (or Enter on it) opens the
  Task sheet. On hover (not on a phone) ☀ Today and 📅 When… sit beside ⋯. The menu (⋯, right-click,
  long-press on a phone) holds Open, Quick edit (E: the row becomes the capture box), When…, Priority…, Move to list…,
  Won't do (Skip to ‹date› on a repeating Task: it moves on and records no occurrence) and Delete. The
  keys (E, D, T, M, 1 to 4, Space, ⇧Space, ↵, ⌫) act on the focused row only, never on one the
  pointer merely rests on (see task-keymap.ts). The focus ring (taskFocus) tints and rings the
  row it is on. In search results `lit` tints the matched words of the title and `snippet` shows the
  note or Comment line that matched, in place of the first line of the notes. A done occurrence shows "↻ occurrence";
  a Won't do Task shows ⊗ and "Won't do" with the time. `hideDate` leaves the date out where the
  group already says it (Today, an Upcoming day). `list` shows the Task's List at the right, for
  views that mix Lists. A done Task is struck through, shows when it was done, and its diamond
  brings it back. An open row of a view that can be arranged (`draggable`) is dragged by its whole
  body with the mouse, to reorder it, to another group or onto the sidebar (see drag.svelte.ts).
-->
<script lang="ts">
  import Calendar from '~icons/lucide/calendar';
  import Check from '~icons/lucide/check';
  import CircleX from '~icons/lucide/circle-x';
  import Clock from '~icons/lucide/clock';
  import Ellipsis from '~icons/lucide/ellipsis';
  import Flag from '~icons/lucide/flag';
  import FolderInput from '~icons/lucide/folder-input';
  import ListChecks from '~icons/lucide/list-checks';
  import MessageSquare from '~icons/lucide/message-square';
  import StickyNote from '~icons/lucide/sticky-note';
  import PencilLine from '~icons/lucide/pencil-line';
  import Repeat from '~icons/lucide/repeat';
  import SkipForward from '~icons/lucide/skip-forward';
  import SquarePen from '~icons/lucide/square-pen';
  import Sun from '~icons/lucide/sun';
  import Trash2 from '~icons/lucide/trash-2';
  import type { SnippetSource, Span, Task, TodoList } from '../../lib/ipc/bindings';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import MarkdownInline from '../../lib/ui/MarkdownInline.svelte';
  import Menu, { type MenuAnchor, type MenuItem } from '../../lib/ui/Menu.svelte';
  import TaskCheckbox from '../../lib/ui/TaskCheckbox.svelte';
  import { formatHm } from '../../lib/ui/when-time';
  import { nextDue, repeatLabel } from '../../lib/ui/when-repeat';
  import { createQuery } from '@tanstack/svelte-query';
  import Undo2 from '~icons/lucide/undo-2';
  import CaptureBox from './CaptureBox.svelte';
  import { todoClock } from './clock.svelte';
  import { closedTime, isOverdue, nextText, rowDateLabel, taskRepeat, timePassed } from './dates';
  import ListGlyph from './ListGlyph.svelte';
  import ListPicker from './ListPicker.svelte';
  import { drag } from './drag.svelte';
  import { checklistProgress, notePreview, plainTitle } from './notes';
  import PriorityMenu from './PriorityMenu.svelte';
  import { todoPrefs } from './prefs.svelte';
  import {
    dateTaskToday,
    deleteTaskWithUndo,
    moveTaskTo,
    setTaskPriority,
    setTaskWhen,
    skipRepeating,
    wontDoTask,
  } from './task-actions';
  import { quickEditing } from './quick-edit.svelte';
  import { listsQuery } from './queries';
  import { taskFocus } from './task-focus.svelte';
  import { taskRows } from './task-keys.svelte';
  import { taskSheet } from './task-sheet.svelte';
  import { RowTouch } from './row-touch.svelte';
  import { priorityTone } from './tasks';
  import { understoodMarks } from './understood';
  import WhenHost from './WhenHost.svelte';

  interface Props {
    task: Task;
    ontick: (task: Task) => void;
    /** Ticking a done Task's diamond. */
    onuntick?: (task: Task) => void;
    hideDate?: boolean;
    list?: TodoList | null;
    /** The mouse can pick the row up (an open Task in a view that can be arranged). */
    draggable?: boolean;
    /** Search results: the matched words of the title, folded. */
    lit?: ReadonlySet<string>;
    /** Search results: the note or Comment line that matched, under the title. */
    snippet?: { spans: Span[]; source: SnippetSource };
  }
  let {
    task,
    ontick,
    onuntick,
    hideDate = false,
    list = null,
    draggable = false,
    lit,
    snippet,
  }: Props = $props();

  let row = $state<HTMLElement | null>(null);
  let more = $state<HTMLButtonElement | null>(null);
  let whenButton = $state<HTMLButtonElement | null>(null);
  let menuOpen = $state(false);
  let menuAnchor = $state.raw<MenuAnchor | null>(null);
  let pickerOpen = $state(false);
  let whenOpen = $state(false);
  let priorityOpen = $state(false);

  // The Task keys find this row, and the pickers D and M open, through the registry.
  $effect(() =>
    taskRows.register(task, {
      when: () => (whenOpen = true),
      moveToList: () => (pickerOpen = true),
    }),
  );
  const ringed = $derived(taskFocus.id === task.id);

  /** Focus that arrived by keyboard puts the ring on this row; a click does not. */
  function onfocusin(e: FocusEvent) {
    const target = e.target as Element;
    if (target.matches(':focus-visible')) taskFocus.id = task.id;
  }

  const isOpen = $derived(task.state === 'open');
  const quickEditOpen = $derived(quickEditing.id === task.id && task.state === 'open');
  // The words capture understood in the title are underlined while the fields still match them.
  const sidebar = createQuery(() => ({
    ...listsQuery(),
    enabled: task.understood !== null && task.understood.includes('"list"'),
  }));
  const marks = $derived(
    task.understood === null
      ? []
      : understoodMarks(task, {
          today: todoClock.today,
          timeFormat: todoPrefs.timeFormat,
          listName: (id) => sidebar.data?.lists.find((e) => e.list.id === id)?.list.name ?? null,
        }),
  );
  const wont = $derived(task.state === 'wont');
  const preview = $derived(notePreview(task.notes));
  const steps = $derived(checklistProgress(task.notes));
  const overdue = $derived(isOverdue(task, todoClock.today));
  const late = $derived(timePassed(task, todoClock.today, todoClock.hm));
  const tone = $derived(
    isOpen && task.priority < 4 ? priorityTone(task.priority) : overdue ? 'overdue' : 'plain',
  );
  const showDate = $derived(isOpen && task.due_date !== null && !hideDate);
  const showTime = $derived(isOpen && task.due_time !== null);
  const repeat = $derived(isOpen ? taskRepeat(task) : null);
  const occurrence = $derived(task.state === 'done' && task.repeat_of !== null);
  /** Where "Skip" would send the Task. */
  const skipTo = $derived(
    repeat && task.due_date ? nextDue(repeat, task.due_date, todoClock.today) : null,
  );
  const closed = $derived(
    task.state !== 'open' && task.closed_at !== null
      ? closedTime(task.closed_at, todoPrefs.timeFormat)
      : null,
  );

  // The menu hands focus back as it closes; what it opens comes after that.
  const afterMenu = (open: () => void) => () => void Promise.resolve().then(open);

  const items = $derived<MenuItem[]>([
    {
      label: 'Open',
      icon: SquarePen,
      shortcut: '↵',
      onselect: () => taskSheet.open(task.id),
    },
    ...(isOpen
      ? [
          {
            label: 'Quick edit',
            icon: PencilLine,
            shortcut: 'E',
            onselect: afterMenu(() => quickEditing.start(task.id)),
          },
        ]
      : []),
    ...(isOpen
      ? [
          {
            label: 'When…',
            icon: Calendar,
            shortcut: 'D',
            separatorBefore: true,
            onselect: afterMenu(() => (whenOpen = true)),
          },
          {
            label: 'Priority…',
            icon: Flag,
            shortcut: '1–4',
            onselect: afterMenu(() => (priorityOpen = true)),
          },
        ]
      : []),
    {
      label: 'Move to list…',
      icon: FolderInput,
      shortcut: 'M',
      separatorBefore: !isOpen,
      onselect: afterMenu(() => (pickerOpen = true)),
    },
    ...(skipTo
      ? [
          {
            label: `Skip to ${nextText(skipTo, todoClock.today)}`,
            icon: SkipForward,
            shortcut: '⇧Space',
            onselect: () => void skipRepeating(task),
          },
        ]
      : isOpen
        ? [
            {
              label: 'Won’t do',
              icon: CircleX,
              shortcut: '⇧Space',
              onselect: () => void wontDoTask(task),
            },
          ]
        : []),
    {
      label: 'Delete',
      icon: Trash2,
      shortcut: '⌫',
      danger: true,
      separatorBefore: true,
      onselect: () => void deleteTask(),
    },
  ]);

  const openSheet = () => taskSheet.open(task.id);

  /** Deleting hands the ring to the next row, as the Journal does. */
  async function deleteTask() {
    const next = ringed ? taskFocus.neighbour(task.id) : null;
    if ((await deleteTaskWithUndo(task)) && ringed) taskFocus.keepAfter(task.id, next);
  }

  function openMenu(anchor: MenuAnchor) {
    menuAnchor = anchor;
    menuOpen = true;
  }

  function oncontextmenu(e: MouseEvent) {
    e.preventDefault();
    // A finger held on the row asks for this too (Android's long-press): the hold has its own menu.
    if (touch.touching) return;
    // The keyboard's menu key reports no position: hang it off the row.
    openMenu(e.clientX === 0 && e.clientY === 0 && row ? row : { x: e.clientX, y: e.clientY });
  }

  // A finger: slide right to complete (or reopen), left for When; hold to lift the row, then lift
  // the finger for the menu or move it to drag the row into place.
  const touch = new RowTouch({
    sides: () => ({ right: true, left: isOpen }),
    onright: () => (isOpen ? ontick(task) : onuntick?.(task)),
    onleft: () => (whenOpen = true),
    onhold: (e) =>
      drag.armTouch(e, task, () => openMenu({ x: e.clientX, y: e.clientY }), draggable && isOpen),
  });
  const touchLike = (e: PointerEvent) => e.pointerType === 'touch' || e.pointerType === 'pen';

  const hover =
    'opacity-0 group-hover:opacity-100 focus-visible:opacity-100 phone:hidden -my-0.5';
</script>

{#if quickEditOpen}
  <CaptureBox
    mode="edit"
    {task}
    class="-mx-0.5 my-1 phone:mx-0"
    onclose={() => quickEditing.stop()}
    onsaved={() => quickEditing.stop()}
  />
{:else}
<div class="swipe relative phone:overflow-hidden phone:rounded-sm">
  {#if touch.dx !== 0}
    <!-- What the slide reveals: Done on the left (Reopen for a closed Task), When on the right. -->
    <div
      aria-hidden="true"
      class="absolute inset-y-0 flex w-1/2 items-center gap-1.5 px-4 text-sm font-semibold
        {touch.dx > 0 ? 'left-0 bg-hue-todo text-on-accent' : 'right-0 justify-end bg-warning text-on-warning'}"
    >
      {#if touch.dx > 0}
        {#if isOpen}<Check class="size-5" />Done{:else}<Undo2 class="size-5" />Reopen{/if}
      {:else}
        When<Calendar class="size-5" />
      {/if}
    </div>
  {/if}
<div
  bind:this={row}
  role="presentation"
  data-task-row={task.id}
  {oncontextmenu}
  {onfocusin}
  onclickcapture={(e) => {
    if (!touch.swallowsClick) return;
    e.stopPropagation();
    e.preventDefault();
  }}
  onpointerdown={(e) => {
    if (!touchLike(e)) {
      if (draggable && isOpen) drag.pressTask(e, task);
    } else touch.down(e);
  }}
  onpointermove={(e) => touchLike(e) && touch.move(e)}
  onpointerup={(e) => touchLike(e) && touch.up(e)}
  onpointercancel={() => touch.cancel()}
  style:transform={touch.dx !== 0 ? `translateX(${touch.dx}px)` : undefined}
  style:transition-property={touch.following ? 'background-color' : undefined}
  class="group grid grid-cols-[26px_1fr_auto] items-start gap-x-2 rounded-sm px-2 py-[7px] transition-[background-color,transform] duration-(--dur) ease-out
    hover:bg-hover phone:grid-cols-[30px_1fr_auto] phone:px-1.5 phone:py-2.5 phone:touch-pan-y phone:select-none phone:[-webkit-touch-callout:none]
    {touch.dx !== 0 ? 'bg-bg' : ''}
    {touch.lifted ? 'bg-hover shadow-card' : ''} {drag.draggedId === task.id ? 'opacity-40' : ''}
    {ringed ? 'bg-hover outline-[1.5px] outline-accent shadow-[0_0_0_3px_var(--ring)]' : ''}"
>
  <TaskCheckbox
    checked={!isOpen}
    {wont}
    label={isOpen ? `Complete ${plainTitle(task.title)}` : `Reopen ${plainTitle(task.title)}`}
    {tone}
    onchange={() => (isOpen ? ontick(task) : onuntick?.(task))}
    class="mt-px"
  />
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_tabindex -- Enter is handled by the key map; the title may hold links, so it is not a button element -->
  <div
    class="min-w-0 cursor-pointer pt-px"
    data-bare
    role="button"
    tabindex="0"
    aria-label="Open {plainTitle(task.title)}"
    onclick={(e) => !window.getSelection()?.toString() && !(e.target as Element).closest('a') && openSheet()}
  >
    <span
      class="leading-[1.45] [overflow-wrap:anywhere] {isOpen ? '' : wont ? 'text-faint line-through' : 'text-muted line-through'}"
    >
      <MarkdownInline source={task.title} {marks} {lit} />
    </span>
    {#if snippet}
      <div class="mt-px flex items-center gap-1 truncate text-[0.78rem] leading-[1.4] text-faint">
        {#if snippet.source === 'comment'}<MessageSquare class="size-3 shrink-0" />{:else}<StickyNote class="size-3 shrink-0" />{/if}
        <span class="truncate">
          {#each snippet.spans as span, k (k)}{#if span.matched}<mark class="rounded-[3px] bg-[color-mix(in_oklch,var(--hue-todo)_28%,transparent)] px-px text-inherit">{span.text}</mark>{:else}{span.text}{/if}{/each}
        </span>
      </div>
    {:else if preview}
      <div class="mt-px truncate text-[0.78rem] leading-[1.4] text-faint">{preview}</div>
    {/if}
    {#if showDate || showTime || closed || repeat || occurrence || steps || task.comment_count > 0}
      <div class="mt-px flex flex-wrap items-center gap-x-2.5 text-xs text-faint">
        {#if showDate && task.due_date}
          <span
            class="inline-flex items-center gap-1 {overdue
              ? 'text-danger'
              : task.due_date === todoClock.today
                ? 'text-hue-todo'
                : ''}"
          >
            <Calendar class="size-3" />{rowDateLabel(task.due_date, todoClock.today)}
          </span>
        {/if}
        {#if showTime && task.due_time}
          <span class="inline-flex items-center gap-1 {overdue || late ? 'text-danger' : ''}">
            <Clock class="size-3" />{formatHm(task.due_time, todoPrefs.timeFormat)}
          </span>
        {/if}
        {#if repeat}
          <span class="inline-flex items-center gap-1"><Repeat class="size-3" />{repeatLabel(repeat)}</span>
        {/if}
        {#if closed}
          <span class="inline-flex items-center gap-1">
            {#if wont}<CircleX class="size-3" />Won’t do · {closed}{:else}<Check class="size-3" />{closed}{/if}
          </span>
        {/if}
        {#if occurrence}
          <span class="inline-flex items-center gap-1"><Repeat class="size-3" />occurrence</span>
        {/if}
        {#if steps}
          <span class="inline-flex items-center gap-1 tabular-nums" title="Checklist">
            <ListChecks class="size-3" />{steps.done}/{steps.total}
          </span>
        {/if}
        {#if task.comment_count > 0}
          <span class="inline-flex items-center gap-1 tabular-nums" title="Comments">
            <MessageSquare class="size-3" />{task.comment_count}
          </span>
        {/if}
      </div>
    {/if}
  </div>
  <div class="flex items-start gap-0.5">
    {#if list}
      <span class="mt-px mr-1.5 flex max-w-36 items-center gap-1 truncate pt-px text-xs text-faint phone:max-w-24">
        <ListGlyph emoji={list.emoji} class="[&_span]:text-xs [&_svg]:size-3.5" />
        <span class="truncate">{list.name}</span>
      </span>
    {/if}
    {#if isOpen}
      <IconButton
        icon={Sun}
        label="Today"
        shortcut="T"
        size="sm"
        onclick={() => void dateTaskToday(task)}
        class={hover}
      />
      <IconButton
        bind:ref={whenButton}
        icon={Calendar}
        label="When…"
        shortcut="D"
        size="sm"
        onclick={() => (whenOpen = true)}
        class="{hover} {whenOpen ? 'opacity-100' : ''}"
      />
    {/if}
    <IconButton
      bind:ref={more}
      icon={Ellipsis}
      label="Task actions"
      size="sm"
      onclick={() => openMenu(more as HTMLElement)}
      class="{hover} {menuOpen ? 'opacity-100' : ''}"
    />
  </div>
</div>
</div>
{/if}

<Menu bind:open={menuOpen} anchor={menuAnchor} {items} label="Task actions" returnFocus={row} />

<PriorityMenu
  bind:open={priorityOpen}
  anchor={more ?? row}
  current={task.priority}
  returnFocus={row}
  onpick={(p) => void setTaskPriority(task, p)}
/>

<ListPicker
  bind:open={pickerOpen}
  anchor={more ?? row}
  current={task.list_id}
  onpick={(list) => void moveTaskTo(task, list)}
/>

{#if isOpen}
  <WhenHost
    bind:open={whenOpen}
    anchor={whenButton ?? row}
    value={{ date: task.due_date, time: task.due_time, repeat: taskRepeat(task) }}
    exclude={task.id}
    onchange={(next) => void setTaskWhen(task, next.date, next.time, next.repeat ?? null)}
  />
{/if}
