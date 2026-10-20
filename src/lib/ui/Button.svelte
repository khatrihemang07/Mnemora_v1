<!--
  Button: primary | secondary | ghost | danger, sizes sm | md | lg.
  Press scales to 0.96 (N3-A). Renders an <a> when `href` is given.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLAnchorAttributes, HTMLButtonAttributes } from 'svelte/elements';

  type Variant = 'primary' | 'secondary' | 'ghost' | 'danger';
  type Size = 'sm' | 'md' | 'lg';
  interface Props extends Omit<HTMLButtonAttributes, 'children'> {
    variant?: Variant;
    size?: Size;
    href?: HTMLAnchorAttributes['href'];
    children: Snippet;
  }

  let { variant = 'secondary', size = 'md', href, children, class: cls = '', ...rest }: Props =
    $props();

  const variants: Record<Variant, string> = {
    primary: 'bg-accent text-on-accent hover:brightness-110',
    secondary: 'bg-surface text-ink border border-line hover:bg-hover',
    ghost: 'text-ink hover:bg-hover',
    danger: 'bg-danger text-on-danger hover:brightness-110',
  };
  const sizes: Record<Size, string> = {
    sm: 'h-7 px-2.5 text-xs',
    md: 'h-(--row) min-h-8 px-3.5 text-sm',
    lg: 'h-11 px-5 text-base',
  };
  const classes = $derived(
    `inline-flex items-center justify-center gap-1.5 rounded-sm font-semibold whitespace-nowrap cursor-pointer select-none
     transition-[transform,background-color,filter] duration-(--dur) ease-out active:scale-[0.96]
     disabled:opacity-50 disabled:pointer-events-none ${variants[variant]} ${sizes[size]} ${cls}`,
  );
</script>

{#if href}
  <a {href} class={classes}>{@render children()}</a>
{:else}
  <button type="button" class={classes} {...rest}>{@render children()}</button>
{/if}
