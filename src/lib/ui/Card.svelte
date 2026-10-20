<!-- Card: hairline-bordered surface (N4-A). `padded` adds the density-aware padding. -->
<script lang="ts">
  import type { Snippet } from 'svelte';
  interface Props {
    padded?: boolean;
    class?: string;
    children: Snippet;
  }
  let { padded = true, class: cls = '', children }: Props = $props();
</script>

<div class="bg-surface border border-line rounded-md shadow-card {padded ? 'p-(--pad)' : ''} {cls}">
  {@render children()}
</div>
