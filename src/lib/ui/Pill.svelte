<!--
  Pill: a floating action pill ("↑ Today", "Back to results"). `tone="ink"` is the neutral dark pill;
  `tone="tinted"` is a raised pill with its text in `hue` (a Module hue token name, e.g.
  `--hue-journal`). Pass `ondismiss` to add a × beside the label (its `dismissLabel` names it). The
  parent positions it through `class`.
-->
<script lang="ts">
  import type { Component, Snippet } from 'svelte';
  import X from '~icons/lucide/x';
  import IconButton from './IconButton.svelte';

  interface Props {
    tone?: 'ink' | 'tinted';
    hue?: string;
    icon?: Component<{ class?: string }>;
    onclick: () => void;
    ondismiss?: () => void;
    dismissLabel?: string;
    class?: string;
    children: Snippet;
  }
  let {
    tone = 'ink',
    hue = '--accent',
    icon: Icon,
    onclick,
    ondismiss,
    dismissLabel = 'Dismiss',
    class: cls = '',
    children,
  }: Props = $props();

  const look = $derived(
    tone === 'ink'
      ? 'bg-ink text-bg'
      : 'border border-line bg-pop text-(--pill-hue)',
  );
  const hover = $derived(tone === 'tinted' ? 'hover:bg-hover' : '');
</script>

<div
  style="--pill-hue: var({hue})"
  class="inline-flex items-center rounded-full text-xs font-semibold shadow-(--elev-pop) {look} {cls}"
>
  <button
    type="button"
    class="inline-flex cursor-pointer items-center gap-1.5 rounded-full py-1.5 select-none {hover} {ondismiss
      ? 'pr-2 pl-3.5'
      : 'px-3.5'}"
    {onclick}
  >
    {#if Icon}<Icon class="size-3.5" />{/if}
    {@render children()}
  </button>
  {#if ondismiss}
    <IconButton icon={X} label={dismissLabel} size="xs" variant="quiet" class="mr-1" onclick={ondismiss} />
  {/if}
</div>
