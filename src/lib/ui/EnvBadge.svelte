<!-- EnvBadge: orange SANDBOX marker. Production shows nothing. -->
<script lang="ts">
  import type { Environment } from '../ipc/bindings';
  let { env, compact = false }: { env: Environment; compact?: boolean } = $props();
</script>

{#if env === 'sandbox'}
  <span
    title="Sandbox Environment: separate, disposable data"
    class="inline-flex items-center rounded-full bg-warning font-bold tracking-wider text-on-warning
      {compact ? 'size-2.5' : 'h-5 px-2 text-2xs'}"
  >
    {#if !compact}SANDBOX{/if}
  </span>
{/if}
