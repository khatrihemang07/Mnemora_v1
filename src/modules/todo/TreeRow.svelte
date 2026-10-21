<!--
  TreeRow: one destination under "Todo" in the sidebar (a view or a List): glyph, name, and an open
  count at the end. The open one carries the active tint. Right-click (or a long-press) is for the
  List menu. A thin wrapper so the views and the Lists look identical. `dropNav` makes the row take
  a dragged Task (it lights up under the pointer); `list` with `reorderable` lets the List itself be
  dragged to a new place.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { TodoList } from '../../lib/ipc/bindings';
  import { LongPress } from '../../lib/long-press';
  import { drag } from './drag.svelte';

  interface Props {
    label: string;
    active: boolean;
    count?: number;
    /** Shows the count in red (something is Overdue). */
    alert?: boolean;
    onclick: () => void;
    /** Right-click or long-press at a point. */
    onmenu?: (x: number, y: number) => void;
    /** Bind to anchor a menu or picker to the row. */
    ref?: HTMLButtonElement | null;
    glyph: Snippet;
    /** `inbox`, `today` or `list:‹id›`: a dragged Task dropped here goes there. */
    dropNav?: string;
    /** A List row: dragging it (when `reorderable`) moves it among the Lists. */
    list?: TodoList;
    reorderable?: boolean;
  }
  let {
    label,
    active,
    count = 0,
    alert = false,
    onclick,
    onmenu,
    ref = $bindable(),
    glyph,
    dropNav,
    list,
    reorderable = false,
  }: Props = $props();

  const lit = $derived(dropNav !== undefined && drag.nav === dropNav);
  const lifted = $derived(list !== undefined && drag.item?.kind === 'list' && drag.draggedId === list.id);

  const press = new LongPress({ onpress: (x, y) => onmenu?.(x, y) });
  const touchLike = (e: PointerEvent) => e.pointerType === 'touch' || e.pointerType === 'pen';
</script>

<button
  bind:this={ref}
  type="button"
  onclick={() => {
    // The lift after a long-press must not also open the destination.
    if (press.fired) press.fired = false;
    else onclick();
  }}
  aria-current={active ? 'page' : undefined}
  oncontextmenu={(e) => {
    if (!onmenu) return;
    e.preventDefault();
    onmenu(e.clientX, e.clientY);
  }}
  data-drop-nav={dropNav}
  data-list-row={list && reorderable ? list.id : undefined}
  onpointerdown={(e) => {
    if (touchLike(e)) {
      if (onmenu) press.start(e.clientX, e.clientY);
    } else if (list && reorderable) drag.pressList(e, list);
  }}
  onpointermove={(e) => touchLike(e) && press.move(e.clientX, e.clientY)}
  onpointerup={() => press.cancel()}
  onpointercancel={() => press.cancel()}
  class="flex h-[calc(var(--row)-4px)] min-h-8 w-full cursor-pointer items-center gap-2.5 rounded-sm pr-2.5 pl-7.5 text-left text-sm transition-colors
    duration-(--dur) ease-out hover:bg-hover {active ? 'bg-active font-semibold text-ink' : 'text-ink'}
    {lit ? 'bg-hover shadow-[inset_0_0_0_1.5px_var(--hue-todo)]' : ''} {lifted ? 'opacity-40' : ''}"
>
  {@render glyph()}
  <span class="min-w-0 flex-1 truncate">{label}</span>
  {#if count > 0}
    <span class="text-xs tabular-nums {alert ? 'font-semibold text-danger' : 'font-normal text-muted'}">{count}</span>
  {/if}
</button>
