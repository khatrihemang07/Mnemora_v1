<!--
  TaskRow: one Task in a list: the diamond to tick it, then its title (wrapping, never cut) and,
  under it, its date and time (Today teal, Overdue red; a time already behind us red). On hover (not
  on a phone) ☀ Today and 📅 When… sit beside ⋯. The menu (⋯, right-click, long-press on a phone)
  holds "When…", which opens the When picker, "Skip to ‹date›" on a repeating Task (it moves on and
  records nothing) and "Move to list…". A repeating Task shows its rule ("↻ Every 3 days after
  done") under the title; a done occurrence shows "↻ occurrence". `hideDate` leaves the date out
  where the group already says it (Today, an Upcoming day). `list` shows the Task's List at the
  right, for views that mix Lists. A done Task (Completed today) is struck through, shows when it
  was done, and its diamond brings it back.
-->
<script lang="ts">
  import Calendar from '~icons/lucide/calendar';
  import Check from '~icons/lucide/check';
  import Clock from '~icons/lucide/clock';
  import Ellipsis from '~icons/lucide/ellipsis';
  import FolderInput from '~icons/lucide/folder-input';
  import Repeat from '~icons/lucide/repeat';
  import SkipForward from '~icons/lucide/skip-forward';
  import Sun from '~icons/lucide/sun';
  import type { Task, TodoList } from '../../lib/ipc/bindings';
  import { LongPress } from '../../lib/long-press';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import Menu, { type MenuAnchor, type MenuItem } from '../../lib/ui/Menu.svelte';
  import TaskCheckbox from '../../lib/ui/TaskCheckbox.svelte';
  import { formatHm } from '../../lib/ui/when-time';
  import { nextDue, repeatLabel } from '../../lib/ui/when-repeat';
  import { todoClock } from './clock.svelte';
  import { closedTime, isOverdue, nextText, rowDateLabel, taskRepeat, timePassed } from './dates';
  import ListGlyph from './ListGlyph.svelte';
  import ListPicker from './ListPicker.svelte';
  import { todoPrefs } from './prefs.svelte';
  import { dateTaskToday, moveTaskTo, setTaskWhen, skipRepeating } from './task-actions';
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

  const isOpen = $derived(task.state === 'open');
  const overdue = $derived(isOverdue(task, todoClock.today));
  const late = $derived(timePassed(task, todoClock.today, todoClock.hm));
  const showDate = $derived(isOpen && task.due_date !== null && !hideDate);
  const showTime = $derived(isOpen && task.due_time !== null);
  const repeat = $derived(isOpen ? taskRepeat(task) : null);
  const occurrence = $derived(task.state === 'done' && task.repeat_of !== null);
  /** Where "Skip" would send the Task. */
  const skipTo = $derived(
    repeat && task.due_date ? nextDue(repeat, task.due_date, todoClock.today) : null,
  );
  const closed = $derived(
    task.state === 'done' && task.closed_at !== null
      ? closedTime(task.closed_at, todoPrefs.timeFormat)
      : null,
  );

  // The menu hands focus back as it closes; what it opens comes after that.
  const afterMenu = (open: () => void) => () => void Promise.resolve().then(open);

  const items = $derived<MenuItem[]>([
    ...(isOpen
      ? [
          {
            label: 'When…',
            icon: Calendar,
            onselect: afterMenu(() => (whenOpen = true)),
          },
        ]
      : []),
    ...(skipTo
      ? [
          {
            label: `Skip to ${nextText(skipTo, todoClock.today)}`,
            icon: SkipForward,
            onselect: () => void skipRepeating(task),
          },
        ]
      : []),
    {
      label: 'Move to list…',
      icon: FolderInput,
      onselect: afterMenu(() => (pickerOpen = true)),
    },
  ]);

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

<div
  bind:this={row}
  role="presentation"
  {oncontextmenu}
  onpointerdown={(e) => touchLike(e) && !(e.target as Element).closest('button') && press.start(e.clientX, e.clientY)}
  onpointermove={(e) => touchLike(e) && press.move(e.clientX, e.clientY)}
  onpointerup={() => press.cancel()}
  onpointercancel={() => press.cancel()}
  class="group grid grid-cols-[26px_1fr_auto] items-start gap-x-2 rounded-sm px-2 py-[7px] transition-colors duration-(--dur) ease-out
    hover:bg-hover phone:grid-cols-[30px_1fr_auto] phone:px-1.5 phone:py-2.5"
>
  <TaskCheckbox
    checked={!isOpen}
    label={isOpen ? `Complete ${task.title}` : `Reopen ${task.title}`}
    tone={overdue ? 'overdue' : 'plain'}
    onchange={() => (isOpen ? ontick(task) : onuntick?.(task))}
    class="mt-px"
  />
  <div class="min-w-0 pt-px">
    <span class="leading-[1.45] [overflow-wrap:anywhere] {isOpen ? '' : 'text-muted line-through'}">
      {task.title}
    </span>
    {#if showDate || showTime || closed || repeat || occurrence}
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
          <span class="inline-flex items-center gap-1"><Check class="size-3" />{closed}</span>
        {/if}
        {#if occurrence}
          <span class="inline-flex items-center gap-1"><Repeat class="size-3" />occurrence</span>
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

<Menu bind:open={menuOpen} anchor={menuAnchor} {items} label="Task actions" returnFocus={row} />

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
