<!--
  InboxPage: open Tasks that belong to no List, each with its diamond; "+ New task" at the end adds
  inline. "Group:" (None, Priority, Due date) and "Sort:" (Manual, Due date, Priority, Title,
  Recently added) are remembered; grouping happens first and the sort orders each group, and
  dragging is off while sorted. Under them a separate "No date, in a List" group shows undated Tasks
  that already sit in a List: they are not part of the Inbox's count. Ticking removes the Task at
  once and offers Undo.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import { canDrag, GROUP_OPTIONS, GROUP_PREF_KEYS, INBOX_SORT_PREF } from './arrange';
  import { todoClock } from './clock.svelte';
  import TodoActions from './TodoActions.svelte';
  import { inboxQuery, savePreference } from './queries';
  import { useSidebar } from './sidebar.svelte';
  import TaskGroups from './TaskGroups.svelte';
  import ViewOptions from './ViewOptions.svelte';
  import { VIEW_ICONS } from './view-icons';

  const inbox = createQuery(() => inboxQuery(todoClock.today));
  const sidebar = useSidebar();
  const view = $derived(inbox.data);
  /** Tasks with no List: what the Inbox badge counts and what "Inbox zero" is about. */
  const loose = $derived(
    view?.groups
      .filter((g) => g.key.kind !== 'no_date_in_list')
      .reduce((n, g) => n + g.tasks.length, 0) ?? 0,
  );
</script>

<PageHeader title="Inbox" icon={VIEW_ICONS.inbox} hue="--hue-todo">
  {#snippet actions()}<TodoActions />{/snippet}
</PageHeader>
<div class="mx-auto max-w-3xl">
  {#if inbox.isPending}
    <div class="flex flex-col gap-3 py-2">
      <Skeleton class="h-5 w-64" />
      <Skeleton class="h-5 w-48" />
    </div>
  {:else if inbox.isError}
    <ErrorBanner message="The Inbox couldn't load." error={inbox.error} onretry={() => inbox.refetch()} />
  {:else if view}
    <ViewOptions
      class="mb-1 justify-end"
      groupBy={view.group_by}
      groupOptions={GROUP_OPTIONS.inbox}
      ongroup={(next) => void savePreference(GROUP_PREF_KEYS.inbox, next)}
      sort={view.sort}
      onsort={(next) => void savePreference(INBOX_SORT_PREF, next)}
    />
    <TaskGroups
      groups={view.groups}
      today={todoClock.today}
      lists={sidebar.lists}
      showList
      arrangeable={canDrag(view.sort)}
    />
    {#if loose === 0}
      <p class="px-2 pt-1 text-sm text-faint">
        <b class="font-semibold text-muted">Inbox zero.</b> New Tasks without a List land here.
      </p>
    {/if}
  {/if}
</div>
