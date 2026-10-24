<!--
  Chip: a small pill button that opens a picker, e.g. the date chip "Today ▾". Shows an optional icon
  before its label and a chevron after. `tone="warm"` tints it with `hue` (a Module hue token name,
  e.g. `--hue-journal`) to say "not the default". Bind `ref` to anchor a popover to it.
-->
<script lang="ts">
  import type { Component, Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import ChevronDown from '~icons/lucide/chevron-down';

  interface Props extends Omit<HTMLButtonAttributes, 'children'> {
    icon?: Component<{ class?: string }>;
    tone?: 'plain' | 'warm';
    hue?: string;
    ref?: HTMLButtonElement | null;
    children: Snippet;
  }
  let {
    icon: Icon,
    tone = 'plain',
    hue = '--accent',
    ref = $bindable(null),
    children,
    class: cls = '',
    ...rest
  }: Props = $props();
</script>

<button
  bind:this={ref}
  type="button"
  style="--chip-hue: var({hue})"
  class="inline-flex cursor-pointer items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-semibold whitespace-nowrap select-none
    transition-[transform,background-color,border-color] duration-(--dur) ease-out active:scale-[0.96]
    disabled:cursor-default disabled:opacity-60 disabled:active:scale-100
    {tone === 'warm'
    ? 'border-transparent bg-[color-mix(in_oklch,var(--chip-hue)_24%,transparent)] text-[color-mix(in_oklab,var(--chip-hue)_65%,var(--ink))]'
    : 'border-line-strong bg-surface-2 text-ink hover:border-faint'} {cls}"
  {...rest}
>
  {#if Icon}<Icon class="size-3.5" />{/if}
  {@render children()}
  <ChevronDown class="size-3 opacity-70" />
</button>
