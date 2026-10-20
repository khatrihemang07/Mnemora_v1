<!-- IconButton: icon-only button. `label` is required: it is both aria-label and tooltip. -->
<script lang="ts">
  import type { Component } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import Tooltip from './Tooltip.svelte';

  interface Props extends Omit<HTMLButtonAttributes, 'children' | 'aria-label'> {
    icon: Component<{ class?: string }>;
    label: string;
    shortcut?: string;
    variant?: 'ghost' | 'secondary';
    size?: 'sm' | 'md';
    tooltipSide?: 'top' | 'right' | 'bottom' | 'left';
  }
  let {
    icon: Icon,
    label,
    shortcut,
    variant = 'ghost',
    size = 'md',
    tooltipSide = 'bottom',
    class: cls = '',
    ...rest
  }: Props = $props();

  const dim = $derived(size === 'sm' ? 'size-7' : 'size-8');
  const look = $derived(variant === 'secondary' ? 'bg-surface border border-line' : '');
</script>

<Tooltip {label} {shortcut} side={tooltipSide}>
  {#snippet trigger({ props })}
    <button
      type="button"
      {...props}
      {...rest}
      aria-label={label}
      class="inline-flex items-center justify-center rounded-sm text-muted hover:text-ink hover:bg-hover cursor-pointer select-none
        transition-[transform,background-color,color] duration-(--dur) ease-out active:scale-[0.96] disabled:opacity-50 {dim} {look} {cls}"
    >
      <Icon class="size-4" />
    </button>
  {/snippet}
</Tooltip>
