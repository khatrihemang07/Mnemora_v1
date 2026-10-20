<!-- SettingRow: label and hint on the left, control on the right (stacked on the phone). -->
<script lang="ts">
  import type { Snippet } from 'svelte';
  interface Props {
    label: string;
    hint?: string;
    children: Snippet;
  }
  let { label, hint, children }: Props = $props();
</script>

<div class="flex flex-col gap-2 p-(--pad) min-[720px]:flex-row min-[720px]:items-center min-[720px]:justify-between min-[720px]:gap-6">
  <div class="min-w-0">
    <div class="font-medium text-ink">{label}</div>
    {#if hint}<div class="text-sm text-muted">{hint}</div>{/if}
  </div>
  <div class="shrink-0">{@render children()}</div>
</div>
