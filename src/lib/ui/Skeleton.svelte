<!-- Skeleton (N7-B): pulsing placeholder, rendered only if loading lasts longer than 300ms. -->
<script lang="ts">
  import { SKELETON_DELAY_MS } from '../timing';
  let { class: cls = 'h-4 w-full' }: { class?: string } = $props();
  let visible = $state(false);

  $effect(() => {
    const t = setTimeout(() => {
      visible = true;
    }, SKELETON_DELAY_MS);
    return () => clearTimeout(t);
  });
</script>

{#if visible}
  <div
    aria-hidden="true"
    class="rounded-sm bg-surface-2 {cls}"
    style="animation: skeleton-pulse 1.4s ease-in-out infinite"
  ></div>
{/if}
