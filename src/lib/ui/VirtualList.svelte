<!--
  VirtualList: a scrolling list that only renders the rows in view, so thousands of rows stay smooth.
  Wraps @tanstack/svelte-virtual. Rows may differ in height: each is measured once it renders, and
  `estimateSize` only guesses until then. `onnearend` fires while the last rows are in view, to load
  more. `onviewport` reports scroll position. `columnClass` styles the inner column (e.g. a max
  width); a row's own `position: sticky` sticks within that row.
-->
<script lang="ts" generics="T">
  import { createVirtualizer, type SvelteVirtualizer, type VirtualItem } from '@tanstack/svelte-virtual';
  import { get } from 'svelte/store';
  import type { Snippet } from 'svelte';
  import { untrack } from 'svelte';

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
    class: cls = '',
    columnClass = '',
  }: Props = $props();

  let scroller = $state<HTMLDivElement | null>(null);
  let rows = $state.raw<VirtualItem[]>([]);
  let total = $state(0);
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

  function reported() {
    if (scroller) onviewport?.(scroller.scrollTop, scroller.clientHeight);
  }

  /** Back to the first row; smooth unless motion is reduced. */
  export function scrollToTop() {
    const reduce =
      document.documentElement.dataset.reduceMotion === 'true' ||
      window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    scroller?.scrollTo({ top: 0, behavior: reduce ? 'auto' : 'smooth' });
  }
</script>

<div bind:this={scroller} class="overflow-y-auto {cls}" onscroll={reported}>
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
