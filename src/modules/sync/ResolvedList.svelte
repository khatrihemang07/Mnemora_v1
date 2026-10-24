<!--
  ResolvedList: "Resolved automatically", what Sync decided without asking, grouped by day. A log:
  nothing to dismiss, a dot on the rows since the last visit. "Use ‹other› instead" writes the
  other value as an ordinary edit; the folded "Order & preferences" line unfolds with Show.
-->
<script lang="ts">
  import { useQueryClient } from '@tanstack/svelte-query';
  import ArrowUpDown from '~icons/lucide/arrow-up-down';
  import Calendar from '~icons/lucide/calendar';
  import Check from '~icons/lucide/check';
  import FileText from '~icons/lucide/file-text';
  import Flag from '~icons/lucide/flag';
  import Inbox from '~icons/lucide/inbox';
  import List from '~icons/lucide/list';
  import Trash2 from '~icons/lucide/trash-2';
  import Circle from '~icons/lucide/circle';
  import type { Component } from 'svelte';
  import { commands, type ResolvedIcon, type ResolvedRow } from '../../lib/ipc/bindings';
  import { unwrap } from '../../lib/ipc/unwrap';
  import Button from '../../lib/ui/Button.svelte';
  import Card from '../../lib/ui/Card.svelte';
  import DayHeading from '../../lib/ui/DayHeading.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import { groupByDay, timeText } from './conflicts';
  import type { TimeFormat } from '../../lib/time/format';

  interface Props {
    rows: ResolvedRow[];
    timeFormat: TimeFormat;
  }
  let { rows, timeFormat }: Props = $props();

  const client = useQueryClient();
  const groups = $derived(groupByDay(rows, new Date(), (r) => r.at));
  let open = $state<Record<string, boolean>>({});
  let busy = $state<string | null>(null);

  const ICONS: Record<ResolvedIcon, Component<{ class?: string }>> = {
    flag: Flag,
    calendar: Calendar,
    list: List,
    check: Check,
    trash: Trash2,
    text: FileText,
    order: ArrowUpDown,
    inbox: Inbox,
    other: Circle,
  };

  const tint = (module: string) =>
    module === 'journal'
      ? 'bg-[color-mix(in_oklch,var(--hue-journal)_16%,transparent)] text-hue-journal'
      : module === 'todo'
        ? 'bg-[color-mix(in_oklch,var(--hue-todo)_16%,transparent)] text-hue-todo'
        : 'bg-hue-sync-soft text-hue-sync';

  async function useOther(row: ResolvedRow) {
    busy = row.id;
    try {
      await unwrap(commands.syncResolutionUseOther(row.id));
      // It is an ordinary edit: whatever shows that record is stale now.
      await Promise.all(
        [['sync', 'conflicts'], ['todo'], ['journal']].map((queryKey) =>
          client.invalidateQueries({ queryKey }),
        ),
      );
    } catch (e) {
      notifyError(e instanceof Error ? e.message : 'Could not change that');
    } finally {
      busy = null;
    }
  }
</script>

<h2 class="px-1 pt-5 pb-1.5 text-2xs font-semibold tracking-wider text-faint uppercase" data-resolved>
  Resolved automatically
</h2>
{#each groups as group (group.key)}
  <DayHeading name={group.title} small={group.subtitle} />
  <Card class="mt-2" padded={false}>
    <ul>
      {#each group.rows as row (row.id)}
        {@const Icon = ICONS[row.icon]}
        <li
          class="grid grid-cols-[2rem_1fr_auto] items-center gap-x-3 gap-y-0.5 px-(--pad) py-3 not-first:border-t not-first:border-line phone:grid-cols-[2rem_1fr]"
          data-resolved-row={row.id}
          data-kind={row.kind}
        >
          <span class="row-span-2 grid size-8 place-items-center rounded-sm {tint(row.module)}">
            <Icon class="size-4" />
          </span>
          <span class="flex min-w-0 items-center gap-1.5 font-medium">
            {#if row.fresh}
              <span
                class="size-1.5 shrink-0 rounded-full bg-hue-sync"
                role="img"
                aria-label="New since your last visit"
                data-new
              ></span>
            {/if}
            <span class="truncate">{row.title}</span>
          </span>
          <span class="row-span-2 phone:col-start-2 phone:row-span-1 phone:mt-1.5">
            {#if row.kind === 'order_prefs'}
              <Button size="sm" variant="ghost" onclick={() => (open[row.id] = !open[row.id])}>
                {open[row.id] ? 'Hide' : 'Show'}
              </Button>
            {:else if row.used}
              <span class="text-sm text-muted" data-used>{row.used}</span>
            {:else if row.action}
              <Button
                size="sm"
                variant="ghost"
                disabled={busy === row.id}
                onclick={() => void useOther(row)}
              >
                {row.action}
              </Button>
            {/if}
          </span>
          <span class="col-start-2 text-sm text-muted">
            {#if row.kind === 'order_prefs'}
              {row.detail} · {timeText(row.at, timeFormat)}
            {:else}
              {row.detail}
            {/if}
          </span>
          {#if row.kind === 'order_prefs' && open[row.id]}
            <ul class="col-start-2 mt-1 text-sm text-muted" data-changes>
              {#each row.changes as change, i (i)}
                <li>{change}</li>
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
  </Card>
{/each}
