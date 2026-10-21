<!--
  TodoFlyout: what the icon rail shows when you hover (600ms) or right-click the Todo icon: the five
  views, then the Lists, each with its open count. Picking one opens it in Todo and closes the flyout.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import type { NavFlyoutProps } from '../../lib/router';
  import ListGlyph from './ListGlyph.svelte';
  import { todoNav } from './nav.svelte';
  import { listsQuery } from './queries';
  import { VIEW_ICONS } from './view-icons';
  import { listView, SMART_VIEWS, sameView, smartView, type TodoView, viewCount } from './views';

  let { ctx, onclose }: NavFlyoutProps = $props();

  const sidebar = createQuery(() => listsQuery());
  const inside = $derived(ctx.isActive('/todo'));

  function go(view: TodoView) {
    todoNav.go(view);
    ctx.navigate('/todo');
    onclose();
  }

  const row =
    'flex h-8 w-full cursor-pointer items-center gap-2.5 rounded-sm px-2.5 text-left text-sm text-ink transition-colors duration-(--dur) ease-out hover:bg-hover';
  const heading = 'px-2.5 pt-1.5 pb-0.5 text-xs font-medium text-faint';
</script>

<div class="flex max-h-[min(70vh,28rem)] min-w-52 flex-col overflow-y-auto" role="menu" aria-label="Todo">
  <p class={heading}>Todo</p>
  {#each SMART_VIEWS as v (v.id)}
    {@const Icon = VIEW_ICONS[v.id]}
    {@const shown = viewCount(sidebar.data, v.id)}
    <button
      type="button"
      role="menuitem"
      onclick={() => go(smartView(v.id))}
      class="{row} {inside && sameView(todoNav.view, smartView(v.id)) ? 'bg-active font-semibold' : ''}"
    >
      <span class="grid size-[1.15rem] shrink-0 place-items-center text-muted"><Icon class="size-4" /></span>
      <span class="flex-1">{v.label}</span>
      {#if shown.count > 0}<span class="text-xs tabular-nums {shown.alert ? 'font-semibold text-danger' : 'font-normal text-muted'}">{shown.count}</span>{/if}
    </button>
  {/each}
  <div class="my-1 h-px bg-[color-mix(in_oklch,var(--ink)_12%,transparent)]"></div>
  <p class={heading}>Lists</p>
  {#each sidebar.data?.lists ?? [] as entry (entry.list.id)}
    <button
      type="button"
      role="menuitem"
      onclick={() => go(listView(entry.list.id))}
      class="{row} {inside && sameView(todoNav.view, listView(entry.list.id)) ? 'bg-active font-semibold' : ''}"
    >
      <ListGlyph emoji={entry.list.emoji} />
      <span class="min-w-0 flex-1 truncate">{entry.list.name}</span>
      {#if entry.open_count > 0}
        <span class="text-xs font-normal text-muted tabular-nums">{entry.open_count}</span>
      {/if}
    </button>
  {:else}
    <p class="px-2.5 py-1.5 text-sm text-faint">No Lists yet</p>
  {/each}
</div>
