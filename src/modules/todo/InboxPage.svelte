<!--
  InboxPage: open Tasks that belong to no List, in order, each with its diamond; "+ New task" at the
  end adds inline. Ticking removes the Task at once and offers Undo.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import NewTaskButton from './NewTaskButton.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import { inboxQuery } from './queries';
  import TaskRows from './TaskRows.svelte';
  import { VIEW_ICONS } from './view-icons';

  const inbox = createQuery(() => inboxQuery);
</script>

<PageHeader title="Inbox" icon={VIEW_ICONS.inbox} hue="--hue-todo">
  {#snippet actions()}<NewTaskButton />{/snippet}
</PageHeader>
<div class="mx-auto max-w-3xl">
  {#if inbox.isPending}
    <div class="flex flex-col gap-3 py-2">
      <Skeleton class="h-5 w-64" />
      <Skeleton class="h-5 w-48" />
    </div>
  {:else if inbox.isError}
    <ErrorBanner message="The Inbox couldn't load." error={inbox.error} onretry={() => inbox.refetch()} />
  {:else}
    <TaskRows tasks={inbox.data.tasks} empty="Nothing here. Tasks without a List land in the Inbox." />
  {/if}
</div>
