<!--
  VirtualList: a scrolling list that only renders the rows in view, so thousands of rows stay smooth.
  Wraps @tanstack/svelte-virtual. Rows may differ in height: each is measured once it renders, and
  `estimateSize` only guesses until then. `onnearend` fires while the last rows are in view, to load
  more. `onviewport` reports scroll position. `columnClass` styles the inner column (e.g. a max
  width). Rows sit in translated boxes where `position: sticky` misplaces itself, so a pinned
  header is drawn as an overlay instead: rows where `isSticky` holds get `pinned` rendered at the
  top while they are being read, pushed up by the next one.
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
  });

  $effect(() => {
    const last = rows.at(-1);
    if (last && last.index >= items.length - nearEnd) untrack(() => onnearend?.());
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
    onviewport?.(scroller.scrollTop, scroller.clientHeight);
  }

  /** Back to the first row; smooth unless motion is reduced. */
  export function scrollToTop() {
    const reduce =
      document.documentElement.dataset.reduceMotion === 'true' ||
      window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    scroller?.scrollTo({ top: 0, behavior: reduce ? 'auto' : 'smooth' });
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
