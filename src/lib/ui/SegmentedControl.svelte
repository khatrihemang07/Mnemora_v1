<!--
  SegmentedControl: single-choice toggle group, optionally with icons. Never deselects.
  Segments are equal width and fill the row. Where the row is narrow (phone, under 360px) a
  group of more than three options becomes a 2-column grid instead of wrapping unevenly.
  Containment is applied only on the phone, where the control gets the full row width; on
  desktop it keeps its natural width. `twoRows` lays four or more options out as two rows of two at
  any width, for a control in a narrow popover where one row would overprint; a long label is cut
  with an ellipsis rather than spilling into the next segment, unless `wrap` is set: then the label
  runs onto a second line, so a name such as "Local AI Wrapper" reads in full.
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
    twoRows?: boolean;
    /** Lets a long label wrap onto more lines instead of being cut short. */
    wrap?: boolean;
  }
  let { value, options, label, onchange, twoRows = false, wrap = false }: Props = $props();
  const gridWhenNarrow = $derived(options.length > 3);
  const twoByTwo = $derived(twoRows && gridWhenNarrow);
</script>

<div class="min-w-0 max-[719.98px]:@container">
  <ToggleGroup.Root
    type="single"
    {value}
    onValueChange={(v) => v && onchange(v as T)}
    aria-label={label}
    class="grid gap-0.5 rounded-sm bg-surface-2 p-0.5
      {twoByTwo ? 'grid-cols-2' : 'auto-cols-fr grid-flow-col'}
      {gridWhenNarrow && !twoByTwo ? '@max-[360px]:grid-flow-row @max-[360px]:grid-cols-2' : ''}"
  >
    {#each options as o (o.value)}
      <ToggleGroup.Item
        value={o.value}
        class="inline-flex min-w-0 cursor-pointer items-center justify-center gap-1.5 rounded-sm px-2.5 text-sm font-medium text-muted select-none
          transition-[background-color,color,box-shadow] duration-(--dur) ease-out hover:text-ink
          data-[state=on]:bg-surface data-[state=on]:text-ink data-[state=on]:shadow-card
          {wrap ? 'h-auto min-h-[max(1.75rem,calc(var(--row)-8px))] py-1 text-center text-balance' : 'h-[max(1.75rem,calc(var(--row)-8px))] whitespace-nowrap'}"
      >
        {#if o.icon}<o.icon class="size-3.5" />{/if}
        <span class={wrap ? 'min-w-0' : 'truncate'}>{o.label}</span>
      </ToggleGroup.Item>
    {/each}
  </ToggleGroup.Root>
</div>
