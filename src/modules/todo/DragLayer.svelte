<!--
  DragLayer: what floats above the page while something is dragged: a label that follows the pointer
  (the Task's title, or the List's name, and below it what dropping here would change: "→ Sat",
  "→ P1", "→ 🏠 Home") and the line that shows where it would land. Both ignore the pointer.
  `kind` picks which drag this instance draws: Tasks are dragged from the page, Lists from the
  sidebar, and each is mounted where its drag starts.
-->
<script lang="ts">
  import { plainTitle } from './notes';
  import { drag } from './drag.svelte';

  let { kind }: { kind: 'task' | 'list' } = $props();

  const item = $derived(drag.item?.kind === kind ? drag.item : null);
  const name = $derived(
    item?.kind === 'task' ? plainTitle(item.task.title) : item?.kind === 'list' ? item.list.name : '',
  );
</script>

{#if item}
  {#if drag.line}
    <div
      aria-hidden="true"
      class="pointer-events-none fixed z-60 h-0.5 -translate-y-1/2 rounded-full bg-hue-todo"
      style="left: {drag.line.left}px; top: {drag.line.top}px; width: {drag.line.width}px"
    >
      <span class="absolute top-1/2 -left-1 size-2 -translate-y-1/2 rounded-full bg-hue-todo"></span>
    </div>
  {/if}
  <div
    aria-hidden="true"
    class="pointer-events-none fixed top-0 left-0 z-60 max-w-72 rounded-md bg-pop px-3 py-1.5 text-sm shadow-pop"
    style="transform: translate({drag.x + 14}px, {drag.y + 10}px)"
  >
    <div class="truncate">{name}</div>
    {#if drag.summary}
      <div class="mt-px truncate text-xs text-hue-todo">→ {drag.summary}</div>
    {/if}
  </div>
{/if}
