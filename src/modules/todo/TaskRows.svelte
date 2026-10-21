<!--
  TaskRows: the rows of one page of Tasks, with "+ New task" at the end (into `listId`, or the Inbox).
  The page around it owns loading and errors.
-->
<script lang="ts">
  import type { Task } from '../../lib/ipc/bindings';
  import NewTaskRow from './NewTaskRow.svelte';
  import { tickTask } from './task-actions';
  import TaskRow from './TaskRow.svelte';

  interface Props {
    tasks: Task[];
    listId?: string | null;
    empty: string;
  }
  let { tasks, listId = null, empty }: Props = $props();
</script>

<div class="flex flex-col">
  {#each tasks as task (task.id)}
    <TaskRow {task} ontick={tickTask} />
  {/each}
  <NewTaskRow {listId} />
</div>
{#if tasks.length === 0}
  <p class="px-2 pt-1 text-sm text-faint">{empty}</p>
{/if}
