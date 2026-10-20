<!-- EmptyState (N6-A): an icon, one sentence, and an optional action. -->
<script lang="ts">
  import type { Component, Snippet } from 'svelte';
  interface Props {
    icon: Component<{ class?: string }>;
    title: string;
    description?: string;
    action?: Snippet;
  }
  let { icon: Icon, title, description, action }: Props = $props();
</script>

<div class="flex flex-col items-center gap-2 px-6 py-8 text-center">
  <span class="grid size-11 place-items-center rounded-full bg-accent-soft text-accent">
    <Icon class="size-5" />
  </span>
  <p class="font-semibold text-ink">{title}</p>
  {#if description}<p class="max-w-sm text-muted">{description}</p>{/if}
  {#if action}<div class="mt-1">{@render action()}</div>{/if}
</div>
