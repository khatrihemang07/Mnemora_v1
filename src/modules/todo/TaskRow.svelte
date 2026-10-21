<!--
  TaskRow: one Task in a list: the diamond to tick it (its ring shows the Priority), then its title
  (inline Markdown, wrapping, never cut) and, under it, the first line of its notes, then a meta line
  with its date and time (Today teal, Overdue red; a time already behind us red), its Repeat, the
  Checklist progress "1/3" and the number of comments. Clicking the title (or Enter on it) opens the
  Task sheet. On hover (not on a phone) ☀ Today and 📅 When… sit beside ⋯. The menu (⋯, right-click,
  long-press on a phone) holds Open, Quick edit (E: the row becomes the capture box), When…, Priority…, Move to list…,
  Won't do (Skip to ‹date› on a repeating Task: it moves on and records nothing) and Delete. The
  keys 1 to 4 and ⇧Space act on the hovered or focused row. A done occurrence shows "↻ occurrence";
  a Won't do Task shows ⊗ and "Won't do" with the time. `hideDate` leaves the date out where the
  group already says it (Today, an Upcoming day). `list` shows the Task's List at the right, for
  views that mix Lists. A done Task is struck through, shows when it was done, and its diamond
  brings it back.
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
  import PencilLine from '~icons/lucide/pencil-line';
  import Repeat from '~icons/lucide/repeat';
  import SkipForward from '~icons/lucide/skip-forward';
  import SquarePen from '~icons/lucide/square-pen';
  import Sun from '~icons/lucide/sun';
  import Trash2 from '~icons/lucide/trash-2';
  import type { Task, TodoList } from '../../lib/ipc/bindings';
  import { LongPress } from '../../lib/long-press';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import MarkdownInline from '../../lib/ui/MarkdownInline.svelte';
  import Menu, { type MenuAnchor, type MenuItem } from '../../lib/ui/Menu.svelte';
  import TaskCheckbox from '../../lib/ui/TaskCheckbox.svelte';
  import { formatHm } from '../../lib/ui/when-time';
  import { nextDue, repeatLabel } from '../../lib/ui/when-repeat';
  import { createQuery } from '@tanstack/svelte-query';
  import CaptureBox from './CaptureBox.svelte';
  import { todoClock } from './clock.svelte';
  import { closedTime, isOverdue, nextText, rowDateLabel, taskRepeat, timePassed } from './dates';
  import ListGlyph from './ListGlyph.svelte';
  import ListPicker from './ListPicker.svelte';
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
  import { taskRows } from './task-keys.svelte';
  import { taskSheet } from './task-sheet.svelte';
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
  }
  let { task, ontick, onuntick, hideDate = false, list = null }: Props = $props();

  let row = $state<HTMLElement | null>(null);
  let more = $state<HTMLButtonElement | null>(null);
  let whenButton = $state<HTMLButtonElement | null>(null);
  let menuOpen = $state(false);
  let menuAnchor = $state.raw<MenuAnchor | null>(null);
  let pickerOpen = $state(false);
  let whenOpen = $state(false);
  let priorityOpen = $state(false);

  // The keys 1 to 4 and ⇧Space find this row through the registry.
  $effect(() => taskRows.register(task));

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
      danger: true,
      separatorBefore: true,
      onselect: () => void deleteTaskWithUndo(task),
    },
  ]);

  const openSheet = () => taskSheet.open(task.id);

  function openMenu(anchor: MenuAnchor) {
    menuAnchor = anchor;
    menuOpen = true;
  }

  function oncontextmenu(e: MouseEvent) {
    e.preventDefault();
    // The keyboard's menu key reports no position: hang it off the row.
    openMenu(e.clientX === 0 && e.clientY === 0 && row ? row : { x: e.clientX, y: e.clientY });
  }

  const press = new LongPress({ onpress: (x, y) => openMenu({ x, y }) });
  const touchLike = (e: PointerEvent) => e.pointerType === 'touch' || e.pointerType === 'pen';

  const hover =
    'opacity-0 group-hover:opacity-100 focus-visible:opacity-100 phone:hidden -my-0.5';
</script>

{#if quickEditOpen}
  <CaptureBox
    mode="edit"
    {task}
    class="-mx-0.5 my-1"
    onclose={() => quickEditing.stop()}
    onsaved={() => quickEditing.stop()}
  />
{:else}
<div
  bind:this={row}
  role="presentation"
  data-task-row={task.id}
  {oncontextmenu}
  onpointerenter={(e) => !touchLike(e) && (taskRows.hoveredId = task.id)}
  onpointerleave={() => taskRows.hoveredId === task.id && (taskRows.hoveredId = null)}
  onpointerdown={(e) => touchLike(e) && !(e.target as Element).closest('button, a') && press.start(e.clientX, e.clientY)}
  onpointermove={(e) => touchLike(e) && press.move(e.clientX, e.clientY)}
  onpointerup={() => press.cancel()}
  onpointercancel={() => press.cancel()}
  class="group grid grid-cols-[26px_1fr_auto] items-start gap-x-2 rounded-sm px-2 py-[7px] transition-colors duration-(--dur) ease-out
    hover:bg-hover phone:grid-cols-[30px_1fr_auto] phone:px-1.5 phone:py-2.5"
>
  <TaskCheckbox
    checked={!isOpen}
    {wont}
    label={isOpen ? `Complete ${plainTitle(task.title)}` : `Reopen ${plainTitle(task.title)}`}
    {tone}
    onchange={() => (isOpen ? ontick(task) : onuntick?.(task))}
    class="mt-px"
  />
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_tabindex -- Enter is handled; the title may hold links, so it is not a button element -->
  <div
    class="min-w-0 cursor-pointer pt-px outline-none focus-visible:rounded-sm focus-visible:shadow-[0_0_0_2px_var(--ring)]"
    role="button"
    tabindex="0"
    aria-label="Open {plainTitle(task.title)}"
    onclick={(e) => !window.getSelection()?.toString() && !(e.target as Element).closest('a') && openSheet()}
    onkeydown={(e) => e.key === 'Enter' && e.target === e.currentTarget && openSheet()}
  >
    <span
      class="leading-[1.45] [overflow-wrap:anywhere] {isOpen ? '' : wont ? 'text-faint line-through' : 'text-muted line-through'}"
    >
      <MarkdownInline source={task.title} {marks} />
    </span>
    {#if preview}
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
        size="sm"
        onclick={() => void dateTaskToday(task)}
        class={hover}
      />
      <IconButton
        bind:ref={whenButton}
        icon={Calendar}
        label="When…"
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
