<!--
  AllPage: every open Task, grouped by List with the Inbox first. A group's heading opens that List.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import ListGlyph from './ListGlyph.svelte';
  import { todoNav } from './nav.svelte';
  import { allQuery } from './queries';
  import { tickTask } from './task-actions';
  import TaskRow from './TaskRow.svelte';
  import { VIEW_ICONS } from './view-icons';
  import { listView, smartView } from './views';

  const all = createQuery(() => allQuery);
  const InboxIcon = VIEW_ICONS.inbox;
</script>

<PageHeader title="All" icon={VIEW_ICONS.all} hue="--hue-todo" />
<div class="mx-auto max-w-3xl">
  {#if all.isPending}
    <div class="flex flex-col gap-3 py-2">
      <Skeleton class="h-5 w-64" />
      <Skeleton class="h-5 w-48" />
    </div>
  {:else if all.isError}
    <ErrorBanner message="The Tasks couldn't load." error={all.error} onretry={() => all.refetch()} />
  {:else if all.data.groups.length === 0}
    <p class="px-2 pt-1 text-sm text-faint">No open Tasks.</p>
  {:else}
    {#each all.data.groups as group (group.list?.id ?? 'inbox')}
      <section class="mb-4" aria-label={group.list?.name ?? 'Inbox'}>
        <button
          type="button"
          onclick={() => todoNav.go(group.list ? listView(group.list.id) : smartView('inbox'))}
          class="mb-0.5 flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1 text-xs font-medium text-faint transition-colors
            duration-(--dur) ease-out hover:bg-hover hover:text-ink"
        >
          {#if group.list}
            <ListGlyph emoji={group.list.emoji} />
            {group.list.name}
          {:else}
            <span class="grid size-[1.15rem] place-items-center"><InboxIcon class="size-4" /></span>
            Inbox
          {/if}
          <span class="tabular-nums">{group.tasks.length}</span>
        </button>
        {#each group.tasks as task (task.id)}
          <TaskRow {task} ontick={tickTask} />
        {/each}
      </section>
    {/each}
  {/if}
</div>
