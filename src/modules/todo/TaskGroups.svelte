<!--
  TaskGroups: a view's groups, as Rust made them. Each group has its heading (Overdue in red, a
  Priority with its flag, a day in the serif display, a List that opens it, "No date, in a List"),
  its Tasks and, at the end, "+ New task" starting from the group (its day and List; never its
  Priority). The sections carry the markers the drag controller reads (drag.svelte.ts): a group
  that is sorted cannot be dropped into, and its rows are not reordered by ⌥↑/⌥↓.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { TaskGroup, TodoList } from '../../lib/ipc/bindings';
  import { addDefaults, groupHeading, groupTakesTasks } from './arrange';
  import GroupHeader from './GroupHeader.svelte';
  import ListGlyph from './ListGlyph.svelte';
  import { todoNav } from './nav.svelte';
  import NewTaskRow from './NewTaskRow.svelte';
  import { tickTask } from './task-actions';
  import TaskRow from './TaskRow.svelte';
  import { VIEW_ICONS } from './view-icons';
  import { listView, smartView } from './views';

  interface Props {
    groups: readonly TaskGroup[];
    today: string;
    lists: ReadonlyMap<string, TodoList>;
    /** Rows can be reordered by dragging (the manual order is in force). */
    arrangeable?: boolean;
    /** Rows show their List at the right (views that mix Lists; List groups already say it). */
    showList?: boolean;
    /** Today's own group is headed "Today" (under an Overdue group). */
    showToday?: boolean;
    /** Rows leave their date out where the group already says it. */
    hideDate?: (group: TaskGroup) => boolean;
    /** The words of "+ New task" at the end of a group. */
    addLabel?: (group: TaskGroup) => string;
    /** The Overdue group's actions, at the right of its heading ("Move all"). */
    overdueActions?: Snippet;
  }
  let {
    groups,
    today,
    lists,
    arrangeable = true,
    showList = false,
    showToday = false,
    hideDate = () => false,
    addLabel = () => 'New task',
    overdueActions,
  }: Props = $props();

  const InboxIcon = VIEW_ICONS.inbox;
  const listOf = (id: string | null) => (id ? (lists.get(id) ?? null) : null);
  const idOf = (group: TaskGroup) => JSON.stringify(group.key);
  const dateOf = (group: TaskGroup) => (group.key.kind === 'day' ? group.key.date : null);
</script>

{#each groups as group (idOf(group))}
  {@const heading = groupHeading(group.key, { today, lists, showToday })}
  {@const day = dateOf(group)}
  <section
    id={day && day !== today ? `upcoming-${day}` : undefined}
    aria-label={heading.kind === 'label'
      ? heading.label
      : heading.kind === 'day'
        ? heading.name
        : heading.kind === 'list'
          ? (heading.list?.name ?? 'Inbox')
          : 'Tasks'}
    data-task-group
    data-adopt={JSON.stringify(group.adopt)}
    data-drag={arrangeable ? undefined : 'off'}
    class="scroll-mt-20"
  >
    {#if heading.kind === 'label'}
      <GroupHeader
        label={heading.label}
        count={group.tasks.length}
        tone={heading.tone}
        priority={heading.priority}
        icon={heading.icon}
        actions={group.key.kind === 'overdue' ? overdueActions : undefined}
      />
    {:else if heading.kind === 'day'}
      <div class="mt-5.5 mb-1 flex items-baseline gap-2 border-b border-line px-2 pb-1.5">
        <h2 class="font-display text-lg" style="font-weight: var(--display-weight)">{heading.name}</h2>
        <small class="text-xs text-faint">{heading.small}</small>
      </div>
    {:else if heading.kind === 'list'}
      <button
        type="button"
        onclick={() => todoNav.go(heading.list ? listView(heading.list.id) : smartView('inbox'))}
        class="mt-4.5 mb-0.5 flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1 text-xs font-medium text-faint transition-colors
          duration-(--dur) ease-out hover:bg-hover hover:text-ink"
      >
        {#if heading.list}
          <ListGlyph emoji={heading.list.emoji} />
          {heading.list.name}
        {:else}
          <span class="grid size-[1.15rem] place-items-center"><InboxIcon class="size-4" /></span>
          Inbox
        {/if}
        <span class="tabular-nums">{group.tasks.length}</span>
      </button>
    {/if}

    {#each group.tasks as task (task.id)}
      <TaskRow
        {task}
        ontick={tickTask}
        draggable={arrangeable}
        hideDate={hideDate(group)}
        list={showList && group.key.kind !== 'list' ? listOf(task.list_id) : null}
      />
    {/each}
    <div data-group-end>
      {#if groupTakesTasks(group)}
        {@const defaults = addDefaults(group.adopt)}
        <NewTaskRow date={defaults.date} listId={defaults.listId ?? null} label={addLabel(group)} />
      {/if}
    </div>
  </section>
{/each}
