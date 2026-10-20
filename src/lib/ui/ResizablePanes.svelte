<!--
  ResizablePanes: a resizable sidebar beside a main area. Wraps paneforge (which works in
  percent) so callers think in pixels. `width` is the width to show; `onResize` reports
  the user's dragged width in px; double-clicking the divider calls `onReset`.
-->
<script lang="ts">
  import { Pane, PaneGroup, PaneResizer } from 'paneforge';
  import type { Snippet } from 'svelte';
  import { draggedWidth, paneLimits, pxToPercent } from './pane-math';

  let {
    width,
    minWidth,
    maxWidth,
    rail,
    onResize,
    onReset,
    sidebar,
    main,
  }: {
    width: number;
    /** Smallest and largest sidebar width in px a drag can reach. */
    minWidth: number;
    maxWidth: number;
    rail: boolean;
    onResize: (px: number) => void;
    onReset: () => void;
    sidebar: Snippet;
    main: Snippet;
  } = $props();

  let groupWidth = $state(0);
  let pane = $state<ReturnType<typeof Pane> | null>(null);
  const limits = $derived(paneLimits(minWidth, maxWidth, groupWidth));

  function syncPane() {
    const api = pane as unknown as { resize(p: number): void } | null;
    if (api) api.resize(pxToPercent(width, groupWidth));
  }
  $effect(() => {
    void width;
    void groupWidth;
    syncPane();
  });
  function handleResize(percent: number) {
    const px = draggedWidth(percent, groupWidth, width);
    if (px === null) return;
    onResize(px);
    syncPane();
  }
</script>

<div class="h-full min-h-0" bind:clientWidth={groupWidth}>
  {#if groupWidth}
    <PaneGroup direction="horizontal">
      <Pane
        bind:this={pane}
        order={1}
        defaultSize={pxToPercent(width, groupWidth)}
        minSize={limits.min}
        maxSize={limits.max}
        onResize={handleResize}
      >
        {@render sidebar()}
      </Pane>
      <PaneResizer
        ondblclick={onReset}
        data-rail={rail || undefined}
        class="relative w-px shrink-0 bg-line transition-colors duration-(--dur) hover:bg-accent data-[active]:bg-accent after:absolute after:inset-y-0 after:-inset-x-1"
      />
      <Pane order={2}>
        {@render main()}
      </Pane>
    </PaneGroup>
  {/if}
</div>
