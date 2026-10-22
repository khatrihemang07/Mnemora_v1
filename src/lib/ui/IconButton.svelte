<!-- IconButton: icon-only button. `label` is required: it is both aria-label and tooltip. -->
<script lang="ts">
  import type { Component } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import Tooltip from './Tooltip.svelte';

  interface Props extends Omit<HTMLButtonAttributes, 'children' | 'aria-label'> {
    icon: Component<{ class?: string }>;
    label: string;
    shortcut?: string;
    /** `quiet` is the faint dismiss/clear look; `solid` is a filled round button in `hue`; `xs` is a small round button for inside fields and chips. */
    variant?: 'ghost' | 'secondary' | 'quiet' | 'solid';
    /** For `solid`: a Module hue token name (e.g. `--hue-todo`) that fills the round button. */
    hue?: string;
    size?: 'xs' | 'sm' | 'md';
    tooltipSide?: 'top' | 'right' | 'bottom' | 'left';
    /** Bind to anchor a popover to the button. */
    ref?: HTMLButtonElement | null;
  }
  let {
    icon: Icon,
    label,
    shortcut,
    variant = 'ghost',
    hue = '--accent',
    size = 'md',
    tooltipSide = 'bottom',
    ref = $bindable(null),
    class: cls = '',
    ...rest
  }: Props = $props();

  const dim = $derived(
    variant === 'solid'
      ? { xs: 'size-6', sm: 'size-7', md: 'size-8' }[size] + ' rounded-full'
      : { xs: 'size-6 rounded-full', sm: 'size-7 rounded-sm', md: 'size-8 rounded-sm' }[size],
  );
  const look = $derived(
    {
      ghost: 'text-muted',
      secondary: 'text-muted bg-surface border border-line',
      quiet: 'text-faint',
      solid: 'bg-(--icon-hue) text-on-accent hover:brightness-110',
    }[variant],
  );
  const glyph = $derived(size === 'xs' ? 'size-3.5' : 'size-4');
</script>

<Tooltip {label} {shortcut} side={tooltipSide}>
  {#snippet trigger({ props })}
    <button
      bind:this={ref}
      type="button"
      {...props}
      {...rest}
      aria-label={label}
      style="--icon-hue: var({hue})"
      class="inline-flex items-center justify-center {variant === 'solid' ? '' : 'hover:text-ink hover:bg-hover'} cursor-pointer select-none
        transition-[transform,background-color,color] duration-(--dur) ease-out active:scale-[0.96] disabled:opacity-50 {dim} {look} {cls}"
    >
      <Icon class={glyph} />
    </button>
  {/snippet}
</Tooltip>
