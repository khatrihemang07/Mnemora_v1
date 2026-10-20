<!--
  SegmentedControl: single-choice toggle group, optionally with icons. Never deselects.
  Segments are equal width and fill the row. Where the row is narrow (phone, under 360px) a
  group of more than three options becomes a 2-column grid instead of wrapping unevenly.
  Containment is applied only on the phone, where the control gets the full row width; on
  desktop it keeps its natural width.
-->
<script lang="ts" generics="T extends string">
  import { ToggleGroup } from 'bits-ui';
  import type { Component } from 'svelte';

  interface Option {
    value: T;
    label: string;
    icon?: Component<{ class?: string }>;
  }
  interface Props {
    value: T;
    options: Option[];
    label: string;
    onchange: (value: T) => void;
  }
  let { value, options, label, onchange }: Props = $props();
  const gridWhenNarrow = $derived(options.length > 3);
</script>

<div class="min-w-0 max-[719.98px]:@container">
  <ToggleGroup.Root
    type="single"
    {value}
    onValueChange={(v) => v && onchange(v as T)}
    aria-label={label}
    class="grid auto-cols-fr grid-flow-col gap-0.5 rounded-sm bg-surface-2 p-0.5
      {gridWhenNarrow ? '@max-[360px]:grid-flow-row @max-[360px]:grid-cols-2' : ''}"
  >
    {#each options as o (o.value)}
      <ToggleGroup.Item
        value={o.value}
        class="inline-flex h-[max(1.75rem,calc(var(--row)-8px))] cursor-pointer items-center justify-center gap-1.5 rounded-sm px-2.5 text-sm font-medium whitespace-nowrap text-muted select-none
          transition-[background-color,color,box-shadow] duration-(--dur) ease-out hover:text-ink
          data-[state=on]:bg-surface data-[state=on]:text-ink data-[state=on]:shadow-card"
      >
        {#if o.icon}<o.icon class="size-3.5" />{/if}
        {o.label}
      </ToggleGroup.Item>
    {/each}
  </ToggleGroup.Root>
</div>
