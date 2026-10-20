<!--
  TodoPage: the Inbox, the first Todo view. Open Tasks that belong to no List, in order, each with
  its diamond; "+ New task" at the end adds inline. Ticking removes the Task at once and offers Undo.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import SquareCheckBig from '~icons/lucide/square-check-big';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import NewTaskRow from './NewTaskRow.svelte';
  import { inboxQuery } from './queries';
  import { tickTask } from './task-actions';
  import TaskRow from './TaskRow.svelte';

  const inbox = createQuery(() => inboxQuery);
</script>

<div class="selectable h-full overflow-y-auto p-(--pad)">
  <PageHeader title="Inbox" icon={SquareCheckBig} hue="--hue-todo" />
  <div class="mx-auto max-w-3xl">
    {#if inbox.isPending}
      <div class="flex flex-col gap-3 py-2">
        <Skeleton class="h-5 w-64" />
        <Skeleton class="h-5 w-48" />
      </div>
    {:else if inbox.isError}
      <ErrorBanner
        message="The Inbox couldn't load."
        error={inbox.error}
        onretry={() => inbox.refetch()}
      />
    {:else}
      <div class="flex flex-col">
        {#each inbox.data.tasks as task (task.id)}
          <TaskRow {task} ontick={tickTask} />
        {/each}
        <NewTaskRow />
      </div>
      {#if inbox.data.tasks.length === 0}
        <p class="px-2 pt-1 text-sm text-faint">Nothing here. Tasks without a List land in the Inbox.</p>
      {/if}
    {/if}
  </div>
</div>
