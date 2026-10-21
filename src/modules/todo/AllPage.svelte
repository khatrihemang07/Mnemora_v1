<!--
  AllPage: every open Task, grouped by List (the Inbox first) unless "Group:" says Priority, Due
  date or None; each view remembers its own choice. A List's heading opens that List, and
  "+ New task" in a List group adds into it.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import { GROUP_OPTIONS, GROUP_PREF_KEYS } from './arrange';
  import { todoClock } from './clock.svelte';
  import NewTaskButton from './NewTaskButton.svelte';
  import { allQuery, savePreference } from './queries';
  import { useSidebar } from './sidebar.svelte';
  import TaskGroups from './TaskGroups.svelte';
  import ViewOptions from './ViewOptions.svelte';
  import { VIEW_ICONS } from './view-icons';

  const all = createQuery(() => allQuery(todoClock.today));
  const sidebar = useSidebar();
</script>

<PageHeader title="All" icon={VIEW_ICONS.all} hue="--hue-todo">
  {#snippet actions()}<NewTaskButton />{/snippet}
</PageHeader>
<div class="mx-auto max-w-3xl">
  {#if all.isPending}
    <div class="flex flex-col gap-3 py-2">
      <Skeleton class="h-5 w-64" />
      <Skeleton class="h-5 w-48" />
    </div>
  {:else if all.isError}
    <ErrorBanner message="The Tasks couldn't load." error={all.error} onretry={() => all.refetch()} />
  {:else}
    <ViewOptions
      class="mb-1 justify-end"
      groupBy={all.data.group_by}
      groupOptions={GROUP_OPTIONS.all}
      ongroup={(next) => void savePreference(GROUP_PREF_KEYS.all, next)}
    />
    {#if all.data.groups.every((g) => g.tasks.length === 0)}
      <p class="px-2 pt-1 text-sm text-faint">No open Tasks.</p>
    {/if}
    <TaskGroups groups={all.data.groups} today={todoClock.today} lists={sidebar.lists} showList />
  {/if}
</div>
