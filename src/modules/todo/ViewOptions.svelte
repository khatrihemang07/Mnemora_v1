<!--
  ViewOptions: the small "Group: Priority" and "Sort: Manual" buttons on a view, each opening a
  choice menu. The view says what its options are (see arrange.ts); picking one is the caller's to
  save. A Sort that is not Manual shows a different icon and the Todo hue, and turns dragging off.
-->
<script lang="ts">
  import ArrowDownWideNarrow from '~icons/lucide/arrow-down-wide-narrow';
  import ArrowUpDown from '~icons/lucide/arrow-up-down';
  import Rows3 from '~icons/lucide/rows-3';
  import type { GroupBy, TaskSort } from '../../lib/ipc/bindings';
  import Menu, { type MenuItem } from '../../lib/ui/Menu.svelte';
  import { type Option, optionLabel, sortMenuLabel, TASK_SORT_OPTIONS } from './arrange';

  interface Props {
    /** The grouping in force and the choices of this view. */
    groupBy: GroupBy;
    groupOptions: readonly Option<GroupBy>[];
    ongroup: (next: GroupBy) => void;
    /** The sort in force, for views that sort (a List, the Inbox). */
    sort?: TaskSort;
    onsort?: (next: TaskSort) => void;
    class?: string;
  }
  let { groupBy, groupOptions, ongroup, sort, onsort, class: cls = '' }: Props = $props();

  let groupOpen = $state(false);
  let sortOpen = $state(false);
  let groupButton = $state<HTMLButtonElement | null>(null);
  let sortButton = $state<HTMLButtonElement | null>(null);

  const groupItems = $derived<MenuItem[]>(
    groupOptions.map((o) => ({
      label: o.label,
      checked: o.value === groupBy,
      onselect: () => ongroup(o.value),
    })),
  );
  const sortItems = $derived<MenuItem[]>(
    TASK_SORT_OPTIONS.map((o) => ({
      label: sortMenuLabel(o),
      checked: o.value === sort,
      onselect: () => onsort?.(o.value),
    })),
  );

  const button =
    'inline-flex h-6.5 cursor-pointer items-center gap-1.5 rounded-sm px-2 text-xs text-muted transition-colors duration-(--dur) ease-out hover:bg-hover hover:text-ink';
</script>

<div class="flex items-center gap-0.5 {cls}">
  <button
    bind:this={groupButton}
    type="button"
    class="{button} {groupOpen ? 'bg-hover text-ink' : ''}"
    aria-haspopup="menu"
    onclick={() => (groupOpen = true)}
  >
    <Rows3 class="size-3.5" />
    Group: {optionLabel(groupOptions, groupBy)}
  </button>
  {#if sort !== undefined && onsort}
    <button
      bind:this={sortButton}
      type="button"
      class="{button} {sort === 'manual' ? '' : 'text-hue-todo'} {sortOpen ? 'bg-hover' : ''}"
      aria-haspopup="menu"
      onclick={() => (sortOpen = true)}
    >
      {#if sort === 'manual'}<ArrowUpDown class="size-3.5" />{:else}<ArrowDownWideNarrow class="size-3.5" />{/if}
      Sort: {optionLabel(TASK_SORT_OPTIONS, sort)}
    </button>
  {/if}
</div>

<Menu bind:open={groupOpen} anchor={groupButton} items={groupItems} label="Group by" heading="Group by" />
<Menu bind:open={sortOpen} anchor={sortButton} items={sortItems} label="Sort by" heading="Sort by" />
