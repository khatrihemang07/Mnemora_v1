<!-- EmptyState (N6-A): an icon, one sentence, and an optional action. -->
<script lang="ts">
  import type { Component, Snippet } from 'svelte';
  interface Props {
    icon: Component<{ class?: string }>;
    title: string;
    description?: string;
    action?: Snippet;
    /** A Module hue token name (e.g. `--hue-journal`) to tint the icon; the brand accent by default. */
    hue?: string;
  }
  let { icon: Icon, title, description, action, hue }: Props = $props();
</script>

<div class="flex flex-col items-center gap-2 px-6 py-8 text-center">
  <span
    class="grid size-11 place-items-center rounded-full {hue
      ? 'bg-[color-mix(in_oklch,var(--tile)_14%,transparent)] text-(--tile)'
      : 'bg-accent-soft text-accent'}"
    style={hue ? `--tile: var(${hue})` : undefined}
  >
    <Icon class="size-5" />
  </span>
  <p class="font-semibold text-ink">{title}</p>
  {#if description}<p class="max-w-sm text-muted">{description}</p>{/if}
  {#if action}<div class="mt-1">{@render action()}</div>{/if}
</div>
