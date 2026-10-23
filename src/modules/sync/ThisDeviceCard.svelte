<!--
  ThisDeviceCard: "me" on the Devices tab: this device's name, "Listening on Wi-Fi and Tailscale"
  (click for the actual addresses and port, to debug Tailscale without noise by default) and Pair a device.
-->
<script lang="ts">
  import ChevronDown from '~icons/lucide/chevron-down';
  import Laptop from '~icons/lucide/laptop';
  import Plus from '~icons/lucide/plus';
  import Smartphone from '~icons/lucide/smartphone';
  import type { ThisDevice } from '../../lib/ipc/bindings';
  import Button from '../../lib/ui/Button.svelte';
  import Card from '../../lib/ui/Card.svelte';
  import { listeningSummary } from './this-device';
  import { pairing } from './pairing.svelte';

  let { device }: { device: ThisDevice } = $props();

  let expanded = $state(false);
  const Icon = $derived(device.platform === 'phone' ? Smartphone : Laptop);
  const summary = $derived(listeningSummary(device));
</script>

<Card>
  <div class="grid grid-cols-[2.5rem_1fr_auto] items-center gap-x-3 phone:grid-cols-[2.5rem_1fr]">
    <span class="relative grid size-10 shrink-0 place-items-center rounded-md bg-hue-sync-soft text-hue-sync">
      <Icon class="size-5" />
      {#if summary.listening}
        <span class="absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full border-2 border-surface bg-success"></span>
      {/if}
    </span>
    <div class="min-w-0">
      <div class="flex flex-wrap items-center gap-2">
        <span class="font-semibold">{device.name}</span>
        <span class="rounded-full border border-line px-2 text-2xs leading-5 font-medium text-muted">This device</span>
      </div>
      <button
        type="button"
        class="flex items-center gap-1 text-left text-sm text-muted hover:text-ink"
        aria-expanded={expanded}
        disabled={!summary.listening}
        onclick={() => (expanded = !expanded)}
      >
        {summary.text}
        {#if summary.listening}
          <ChevronDown class="size-3.5 transition-transform duration-(--dur) {expanded ? 'rotate-180' : ''}" />
        {/if}
      </button>
    </div>
    <Button variant="primary" class="phone:col-span-full phone:mt-3 phone:w-full phone:justify-center" onclick={() => void pairing.start('show')}>
      <Plus class="size-4" /> Pair a device
    </Button>
  </div>
  {#if expanded && summary.listening}
    <p class="mt-2.5 border-t border-dashed border-line pt-2.5 font-mono text-xs break-words text-muted">{summary.detail}</p>
  {/if}
</Card>
