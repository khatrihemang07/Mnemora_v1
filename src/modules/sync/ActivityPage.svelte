<!--
  ActivityPage: the log of what Sync did, kept forever and shared between devices. Serif day headings
  as in Todo's Done, newest first; filter chips (All · This device · each paired device); a session
  opens into its per-Module breakdown (counts, never content). A VirtualList draws the rows and asks
  Rust for the next page of 100 just before the end shows, so years of lines stay smooth. New lines
  are taken in at once while the list is at the top, otherwise when it is scrolled back there.
-->
<script lang="ts">
  import { createInfiniteQuery, createQuery, useQueryClient } from '@tanstack/svelte-query';
  import type { Component } from 'svelte';
  import ArrowLeftRight from '~icons/lucide/arrow-left-right';
  import Check from '~icons/lucide/check';
  import ChevronDown from '~icons/lucide/chevron-down';
  import Combine from '~icons/lucide/combine';
  import GitMerge from '~icons/lucide/git-merge';
  import Link from '~icons/lucide/link';
  import LogOut from '~icons/lucide/log-out';
  import Pause from '~icons/lucide/pause';
  import Pencil from '~icons/lucide/pencil';
  import Play from '~icons/lucide/play';
  import ShieldAlert from '~icons/lucide/shield-alert';
  import Unlink from '~icons/lucide/unlink';
  import UserPlus from '~icons/lucide/user-plus';
  import WifiOff from '~icons/lucide/wifi-off';
  import { readCore } from '../../lib/settings/core';
  import { settingsQuery } from '../../lib/settings/queries';
  import type { ActivityRow } from '../../lib/ipc/bindings';
  import DayHeading from '../../lib/ui/DayHeading.svelte';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import FilterChips from '../../lib/ui/FilterChips.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import VirtualList from '../../lib/ui/VirtualList.svelte';
  import { moduleContext } from '../../shell/module-context';
  import {
    type ActivityItem,
    CHIP_ALL,
    chipsOf,
    chipStillThere,
    estimate,
    expandable,
    filterOf,
    itemsOf,
    TOP_PX,
  } from './activity';
  import { timeText } from './conflicts';
  import { activityQuery, devicesQuery, watchActivity, whileMounted } from './queries';
  import SyncFrame from './SyncFrame.svelte';

  const devices = createQuery(() => devicesQuery);
  const peers = $derived(devices.data?.peers ?? []);
  let picked = $state(CHIP_ALL);
  // A device that was forgotten cannot stay chosen.
  const chip = $derived(chipStillThere(picked, peers));
  const filter = $derived(filterOf(chip));

  const activity = createInfiniteQuery(() => activityQuery(filter));
  const rows = $derived(activity.data?.pages.flatMap((p) => p.rows) ?? []);
  const items = $derived(itemsOf(rows));

  const settings = createQuery(() => settingsQuery);
  const timeFormat = $derived(readCore(settings.data?.values).timeFormat);
  const client = useQueryClient();

  let open = $state<Record<string, boolean>>({});
  let list = $state<ReturnType<typeof VirtualList<ActivityItem>> | null>(null);
  let atTop = true;
  let stale = false;

  const ICONS: Record<string, Component<{ class?: string }>> = {
    session: ArrowLeftRight,
    paired: Link,
    joined: UserPlus,
    left: LogOut,
    forgot: Unlink,
    renamed: Pencil,
    paused: Pause,
    resumed: Play,
    blocked: ShieldAlert,
    failed: WifiOff,
    conflict_raised: GitMerge,
    conflict_resolved: Check,
    lists_merged: Combine,
  };
  const iconOf = (row: ActivityRow) => ICONS[row.kind] ?? ArrowLeftRight;

  /** Starts over from the newest page, so new lines show without reloading every page scrolled past. */
  function takeInNew() {
    stale = false;
    const key = ['sync', 'activity'];
    client.setQueryData([...key, filter], (data: typeof activity.data) =>
      data ? { pages: data.pages.slice(0, 1), pageParams: data.pageParams.slice(0, 1) } : data,
    );
    void client.invalidateQueries({ queryKey: key });
  }

  $effect(() =>
    whileMounted(() =>
      watchActivity(() => {
        if (atTop) takeInNew();
        else stale = true;
      }),
    ),
  );

  function viewed(top: number) {
    atTop = top < TOP_PX;
    if (atTop && stale) takeInNew();
  }

  function more() {
    if (activity.hasNextPage && !activity.isFetchingNextPage) void activity.fetchNextPage();
  }

  function choose(next: string) {
    picked = next;
    atTop = true;
    list?.releaseAnchor();
  }
</script>

<SyncFrame tab="activity" fill>
  <div class="flex h-full min-h-0 flex-col" data-activity>
    {#if peers.length > 0}
      <FilterChips
        class="mb-2 shrink-0"
        label="Whose activity"
        hue="--hue-sync"
        options={chipsOf(peers)}
        value={chip}
        onchange={choose}
      />
    {/if}
    {#if activity.isError}
      <ErrorBanner error={activity.error} onretry={() => activity.refetch()} />
    {:else if activity.isPending}
      <Skeleton class="h-32 w-full" />
    {:else if items.length === 0}
      <EmptyState
        icon={ArrowLeftRight}
        hue="--hue-sync"
        title="No activity yet"
        description={chip === CHIP_ALL
          ? 'Each time this device syncs with another, it is listed here.'
          : 'Nothing here for that device yet.'}
      />
    {:else}
      <div class="min-h-0 flex-1">
        <VirtualList
          bind:this={list}
          {items}
          getKey={(item) => item.key}
          estimateSize={(item) => estimate(item, item.kind === 'row' && open[item.row.id] === true)}
          onnearend={more}
          nearEnd={40}
          onviewport={viewed}
          class="h-full"
        >
          {#snippet row(item)}
            {#if item.kind === 'day'}
              <DayHeading name={item.title} small={item.subtitle} first={item.first} />
            {:else}
              {@const r = item.row}
              {@const Icon = iconOf(r)}
              <div class="border-b border-line" data-activity-row={r.id} data-kind={r.kind}>
                <div class="flex items-start gap-3 px-2 py-2.5">
                  <span
                    class="mt-px grid size-8 shrink-0 place-items-center rounded-sm bg-hue-sync-soft text-hue-sync"
                  >
                    <Icon class="size-4" />
                  </span>
                  <div class="min-w-0 flex-1 pt-1">
                    {#if expandable(r)}
                      <button
                        type="button"
                        class="flex w-full cursor-pointer items-start gap-1.5 rounded-sm text-left"
                        aria-expanded={open[r.id] === true}
                        onclick={() => (open[r.id] = !open[r.id])}
                      >
                        <span class="min-w-0 flex-1">{@render sentence(r)}</span>
                        <ChevronDown
                          class="mt-0.5 size-4 shrink-0 text-faint transition-transform duration-(--dur) {open[r.id] ? 'rotate-180' : ''}"
                        />
                      </button>
                      {#if open[r.id]}
                        <p class="mt-1 text-sm text-muted" data-breakdown>{r.breakdown}</p>
                      {/if}
                    {:else}
                      {@render sentence(r)}
                      {#if r.link === 'conflicts'}
                        <span class="text-faint"> · </span>
                        <button
                          type="button"
                          class="cursor-pointer rounded-sm font-medium text-hue-sync hover:underline"
                          onclick={() => moduleContext.navigate('/sync/conflicts')}
                        >
                          Review
                        </button>
                      {/if}
                    {/if}
                  </div>
                  <span class="shrink-0 pt-1 text-sm text-faint tabular-nums">
                    {timeText(r.at, timeFormat)}
                  </span>
                </div>
              </div>
            {/if}
          {/snippet}
        </VirtualList>
      </div>
    {/if}
  </div>
</SyncFrame>

{#snippet sentence(r: ActivityRow)}
  <span class="wrap-anywhere">
    {#each r.text as piece, i (i)}{#if piece.strong}<b class="font-semibold">{piece.text}</b>{:else}{piece.text}{/if}{/each}
    {#if r.on}<span class="text-faint"> · on {r.on}</span>{/if}
  </span>
{/snippet}
