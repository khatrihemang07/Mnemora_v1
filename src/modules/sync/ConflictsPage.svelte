<!-- ConflictsPage: "Needs you", the texts two devices rewrote the same way, grouped by day, and below it "Resolved automatically", what Sync decided without asking. Leaving the page marks the log as visited. -->
<script lang="ts">
  import { createQuery, useQueryClient } from '@tanstack/svelte-query';
  import GitMerge from '~icons/lucide/git-merge';
  import NotebookPen from '~icons/lucide/notebook-pen';
  import FilePen from '~icons/lucide/file-pen';
  import { commands } from '../../lib/ipc/bindings';
  import { readCore } from '../../lib/settings/core';
  import { settingsQuery } from '../../lib/settings/queries';
  import Button from '../../lib/ui/Button.svelte';
  import Card from '../../lib/ui/Card.svelte';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import { moduleContext } from '../../shell/module-context';
  import { groupByDay, rowTitle, sidesText } from './conflicts';
  import { conflictsQuery, watchConflicts } from './queries';
  import ResolvedList from './ResolvedList.svelte';
  import SyncFrame from './SyncFrame.svelte';

  const conflicts = createQuery(() => conflictsQuery);
  const rows = $derived(conflicts.data?.needs_you ?? []);
  const groups = $derived(groupByDay(rows));
  const resolved = $derived(conflicts.data?.resolved ?? []);
  const settings = createQuery(() => settingsQuery);
  const timeFormat = $derived(readCore(settings.data?.values).timeFormat);
  const client = useQueryClient();

  // The log counts as seen once the person has left it: the dots stay while they read.
  $effect(
    () => () =>
      void commands
        .syncResolutionsVisited()
        .then(() => client.invalidateQueries({ queryKey: ['sync', 'conflicts'] })),
  );

  $effect(() => {
    let stop: (() => void) | undefined;
    let gone = false;
    void watchConflicts().then((off) => (gone ? off() : (stop = off)));
    return () => {
      gone = true;
      stop?.();
    };
  });
</script>

<SyncFrame tab="conflicts">
  {#if conflicts.isError}
    <ErrorBanner error={conflicts.error} onretry={() => conflicts.refetch()} />
  {:else if conflicts.isPending}
    <Skeleton class="h-32 w-full" />
  {:else}
    {#if rows.length === 0}
      <Card>
        <EmptyState
          icon={GitMerge}
          hue="--hue-sync"
          title="Nothing needs you"
          description="If the same text is rewritten on two devices, both versions wait here for you to choose."
        />
      </Card>
    {:else}
      <h2
        class="px-1 pt-2 pb-1.5 text-2xs font-semibold tracking-wider text-faint uppercase"
        data-needs-you
      >
        Needs you · {rows.length}
      </h2>
      {#each groups as group (group.key)}
        <div class="flex items-baseline gap-2 border-b border-line px-1 pt-3 pb-1.5">
          <b class="font-display text-lg font-semibold">{group.title}</b>
          <small class="text-xs text-faint">{group.subtitle}</small>
        </div>
        <Card class="mt-2" padded={false}>
          <ul>
            {#each group.rows as row (row.id)}
              {@const Icon = row.module === 'journal' ? NotebookPen : FilePen}
              <li
                class="grid grid-cols-[2rem_1fr_auto] items-center gap-x-3 gap-y-0.5 px-(--pad) py-3 not-first:border-t not-first:border-line phone:grid-cols-[2rem_1fr]"
                data-conflict={row.id}
              >
                <span
                  class="row-span-2 grid size-8 place-items-center rounded-sm {row.module === 'journal'
                    ? 'bg-[color-mix(in_oklch,var(--hue-journal)_16%,transparent)] text-hue-journal'
                    : 'bg-[color-mix(in_oklch,var(--hue-todo)_16%,transparent)] text-hue-todo'}"
                >
                  <Icon class="size-4" />
                </span>
                <span class="min-w-0 truncate font-medium">{rowTitle(row.subject)}</span>
                <span class="row-span-2 phone:col-start-2 phone:row-span-1 phone:mt-1.5">
                  <Button size="sm" onclick={() => moduleContext.navigate(`/sync/conflicts/${row.id}`)}>
                    Compare
                  </Button>
                </span>
                <span class="col-start-2 text-sm text-muted">{sidesText(row)}</span>
              </li>
            {/each}
          </ul>
        </Card>
      {/each}
    {/if}
    {#if resolved.length > 0}
      <ResolvedList rows={resolved} {timeFormat} />
    {/if}
  {/if}
</SyncFrame>
