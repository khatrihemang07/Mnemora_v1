<!--
  VirtualList: a scrolling list that only renders the rows in view, so thousands of rows stay smooth.
  Wraps @tanstack/svelte-virtual. Rows may differ in height: each is measured once it renders, and
  `estimateSize` only guesses until then. `onnearend` fires while the last rows are in view, to load
  more; `onnearstart` while the first rows are (more above). `onviewport` reports scroll position.
  `columnClass` styles the inner column (e.g. a max width). Rows sit in translated boxes where `position: sticky` misplaces itself, so a pinned
  header is drawn as an overlay instead: rows where `isSticky` holds get `pinned` rendered at the
  top while they are being read, pushed up by the next one. When rows are added or removed above
  the ones being read, the scroll position follows the rows, so the view stays put. `scrollToIndex`,
  `offsetOf` and `indexAt` let a parent jump to, and measure, rows.
-->
<script lang="ts" generics="T">
  import { createVirtualizer, type SvelteVirtualizer, type VirtualItem } from '@tanstack/svelte-virtual';
  import { get } from 'svelte/store';
  import type { Snippet } from 'svelte';
  import { untrack } from 'svelte';
  import { pinnedRow } from './pinned-row';

  interface Props {
    /** One entry per row. */
    items: T[];
    /** A stable key per row, so measured heights follow rows when items are added above. */
    getKey: (item: T, index: number) => string;
    /** A first guess of a row's height in px. */
    estimateSize: (item: T, index: number) => number;
    row: Snippet<[T, number]>;
    /** Called while one of the last `nearEnd` rows is rendered. May fire repeatedly; guard it. */
    onnearend?: () => void;
    nearEnd?: number;
    /** Like `onnearend`, for the first `nearStart` rows: load more above. New rows above never move the view. */
    onnearstart?: () => void;
    nearStart?: number;
    onviewport?: (scrollTop: number, height: number) => void;
    isSticky?: (item: T) => boolean;
    pinned?: Snippet<[T]>;
    class?: string;
    columnClass?: string;
  }
  let {
    items,
    getKey,
    estimateSize,
    row,
    onnearend,
    nearEnd = 3,
    onnearstart,
    nearStart = 3,
    onviewport,
    isSticky,
    pinned,
    class: cls = '',
    columnClass = '',
  }: Props = $props();

  let scroller = $state<HTMLDivElement | null>(null);
  let rows = $state.raw<VirtualItem[]>([]);
  let total = $state(0);
  let scrollTop = $state(0);
  let pinnedHeight = $state(0);
  let outerWidth = $state(0);
  let innerWidth = $state(0);
  // The scrollbar's width, so the overlay lines up with the rows under it.
  const gutter = $derived(outerWidth - innerWidth);
  let instance: SvelteVirtualizer<HTMLDivElement, HTMLElement> | undefined;
  /** The first row in view and how far into it the view starts, to hold the view across inserts. */
  let anchor: { key: string | number | bigint; index: number; delta: number } | null = null;

  const options = () => ({
    count: items.length,
    getScrollElement: () => scroller,
    estimateSize: (i: number) => {
      const item = items[i];
      return item === undefined ? 0 : estimateSize(item, i);
    },
    getItemKey: (i: number) => {
      const item = items[i];
      return item === undefined ? i : getKey(item, i);
    },
    overscan: 4,
  });

  const virtualizer = createVirtualizer<HTMLDivElement, HTMLElement>(untrack(options));

  // Mirror the store into state once mounted (the scroll element exists by then). Untracked: the
  // store's own callbacks read `items`, and re-subscribing on every change would restart it.
  $effect(() =>
    untrack(() =>
      virtualizer.subscribe((v) => {
        instance = v;
        rows = v.getVirtualItems();
        total = v.getTotalSize();
      }),
    ),
  );
  // Push new items/sizes in. `get` reads without subscribing, so this cannot loop.
  $effect(() => {
    void items;
    void estimateSize;
    void getKey;
    get(virtualizer).setOptions(options());
    untrack(holdAnchor);
  });

  $effect(() => {
    const last = rows.at(-1);
    if (last && last.index >= items.length - nearEnd) untrack(() => onnearend?.());
  });

  $effect(() => {
    const first = rows[0];
    if (first && first.index <= nearStart) untrack(() => onnearstart?.());
  });

  function measure(node: HTMLElement) {
    instance?.measureElement(node);
    return {
      destroy() {
        instance?.measureElement(null);
      },
    };
  }

  const pin = $derived.by(() => {
    if (!isSticky || !pinned) return null;
    const sticky = (i: number) => {
      const item = items[i];
      return item !== undefined && isSticky(item);
    };
    return pinnedRow(rows, scrollTop, sticky, pinnedHeight);
  });

  function reported() {
    if (!scroller) return;
    scrollTop = scroller.scrollTop;
    const first = rows.find((r) => r.start + r.size > scrollTop);
    anchor = first ? { key: first.key, index: first.index, delta: scrollTop - first.start } : null;
    onviewport?.(scroller.scrollTop, scroller.clientHeight);
  }

  /**
   * Rows inserted or removed above the one being read move it; scroll by the same amount. Only an
   * index change counts: a row resizing is already followed by the virtualizer. Whatever the new
   * rows' real size turns out to be, the virtualizer corrects once they are measured.
   */
  function holdAnchor() {
    if (!anchor || !scroller || !instance) return;
    const index = items.findIndex((item, i) => getKey(item, i) === anchor?.key);
    if (index < 0 || index === anchor.index) return;
    const start = instance.getOffsetForIndex(index, 'start')?.[0];
    if (start === undefined) return;
    scroller.scrollTop = start + anchor.delta;
    anchor.index = index;
  }

  const reducedMotion = () =>
    document.documentElement.dataset.reduceMotion === 'true' ||
    window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  /**
   * Brings row `index` to the top. Rows jumped over keep their estimated heights, so the target's
   * start is exact however far the jump. Smooth (unless motion is reduced) for hops back.
   */
  export function scrollToIndex(index: number, smooth = false) {
    // An instant jump holds its row from now on: rows added above before the first scroll event
    // must not move it. (A smooth one is followed by the scroll events it causes.)
    const item = items[index];
    if (!smooth && item !== undefined) anchor = { key: getKey(item, index), index, delta: 0 };
    scroller?.scrollTo({ top: offsetOf(index), behavior: smooth && !reducedMotion() ? 'smooth' : 'auto' });
  }

  /** Where row `index` starts, in px from the top of the list. */
  export function offsetOf(index: number): number {
    return instance?.getOffsetForIndex(index, 'start')?.[0] ?? 0;
  }

  /** The row at `offset` px from the top of the list (the nearest rendered one when between). */
  export function indexAt(offset: number): number {
    const row = rows.find((r) => r.start <= offset && offset < r.start + r.size) ?? rows[0];
    return row?.index ?? 0;
  }
</script>

<div class="relative h-full">
  <div
    bind:this={scroller}
    bind:offsetWidth={outerWidth}
    bind:clientWidth={innerWidth}
    class="overflow-y-auto {cls}"
    onscroll={reported}
  >
    <div class="relative {columnClass}" style="height: {total}px">
      {#each rows as r (r.key)}
        {@const item = items[r.index]}
        {#if item !== undefined}
          <div
            data-index={r.index}
            use:measure
            class="absolute top-0 left-0 w-full"
            style="transform: translateY({r.start}px)"
          >
            {@render row(item, r.index)}
          </div>
        {/if}
      {/each}
    </div>
  </div>
  {#if pin && pinned && items[pin.index] !== undefined}
    <div
      aria-hidden="true"
      class="pointer-events-none absolute top-0 left-0 z-[3] overflow-hidden {cls}"
      style="right: {gutter}px; height: auto"
    >
      <div
        class={columnClass}
        style="transform: translateY({pin.offset}px)"
        bind:clientHeight={pinnedHeight}
      >
        {@render pinned(items[pin.index] as T)}
      </div>
    </div>
  {/if}
</div>
