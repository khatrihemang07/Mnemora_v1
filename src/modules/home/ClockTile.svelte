<!-- ClockTile: live clock; seconds render smaller. Tabular numerals keep it from jittering. -->
<script lang="ts">
  import Card from '../../lib/ui/Card.svelte';
  import { clockParts } from '../../lib/time/format';
  import type { TimeFormat } from '../../lib/time/format';

  let { now, format }: { now: Date; format: TimeFormat } = $props();
  const parts = $derived(clockParts(now, format));
</script>

<Card class="flex flex-col items-center justify-center gap-1">
  <div class="text-xs font-medium tracking-wider text-faint uppercase">Local time</div>
  <time
    datetime={now.toISOString()}
    class="flex items-baseline gap-1 tabular-nums"
    style="letter-spacing: -0.04em"
  >
    <span class="text-5xl font-medium text-ink">{parts.hm}</span>
    <span class="text-xl text-muted">:{parts.seconds}</span>
    {#if parts.period}<span class="ml-1 text-base text-muted" style="letter-spacing: 0">{parts.period}</span>{/if}
  </time>
</Card>
