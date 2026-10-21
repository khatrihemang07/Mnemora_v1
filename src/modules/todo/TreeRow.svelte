<!--
  TreeRow: one destination under "Todo" in the sidebar (a view or a List): glyph, name, and an open
  count at the end. The open one carries the active tint. Right-click (or a long-press) is for the
  List menu. A thin wrapper so the views and the Lists look identical.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import { LongPress } from '../../lib/long-press';

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
  }
  let { label, active, count = 0, alert = false, onclick, onmenu, ref = $bindable(), glyph }: Props = $props();

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
  onpointerdown={(e) => touchLike(e) && onmenu && press.start(e.clientX, e.clientY)}
  onpointermove={(e) => touchLike(e) && press.move(e.clientX, e.clientY)}
  onpointerup={() => press.cancel()}
  onpointercancel={() => press.cancel()}
  class="flex h-[calc(var(--row)-4px)] min-h-8 w-full cursor-pointer items-center gap-2.5 rounded-sm pr-2.5 pl-7.5 text-left text-sm transition-colors
    duration-(--dur) ease-out hover:bg-hover {active ? 'bg-active font-semibold text-ink' : 'text-ink'}"
>
  {@render glyph()}
  <span class="min-w-0 flex-1 truncate">{label}</span>
  {#if count > 0}
    <span class="text-xs tabular-nums {alert ? 'font-semibold text-danger' : 'font-normal text-muted'}">{count}</span>
  {/if}
</button>
