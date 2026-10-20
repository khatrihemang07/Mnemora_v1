<!-- Kbd: a keyboard shortcut hint. Pass "⌘K" as one string or keys as an array. -->
<script lang="ts">
  let { keys }: { keys: string | string[] } = $props();
  const list = $derived(Array.isArray(keys) ? keys : [keys]);
</script>

<span class="inline-flex gap-0.5">
  {#each list as k}
    <kbd
      class="font-mono text-xs leading-none min-w-5 h-5 px-1 inline-flex items-center justify-center rounded-sm border border-line bg-surface-2 text-muted"
      >{k}</kbd
    >
  {/each}
</span>
