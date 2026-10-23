<!--
  PeerRow: one paired device. Its platform icon wears a status dot; under the name a plain status word,
  how long ago it synced (the full time on hover), what the last session moved and how it was reached.
  While it syncs the line reads "Syncing… 120 of 340 changes" and a hairline bar fades in, but only if the
  sync is still running after 600ms.
-->
<script lang="ts">
  import { onDestroy } from 'svelte';
  import Laptop from '~icons/lucide/laptop';
  import Smartphone from '~icons/lucide/smartphone';
  import type { PeerRow } from '../../lib/ipc/bindings';
  import { clock } from '../../lib/time/clock.svelte';
  import ProgressBar from '../../lib/ui/ProgressBar.svelte';
  import { BarGate } from './bar-gate';
  import { deviceSheet } from './device-sheet.svelte';
  import { absoluteTime, rowParts } from './peer-status';
  import { CHIP, DOT } from './peer-style';

  let { peer }: { peer: PeerRow } = $props();

  // A relative time moves on while the page is open.
  $effect(() => clock.subscribe());

  const Icon = $derived(peer.platform === 'phone' ? Smartphone : Laptop);
  const parts = $derived(rowParts(peer, clock.now));
  const detail = $derived(parts.slice(1).join(' · '));
  const hover = $derived(peer.last_synced_at === null ? undefined : absoluteTime(peer.last_synced_at));
  const syncing = $derived(peer.state === 'syncing');
  const fraction = $derived(
    peer.progress === null || peer.progress.total === 0 ? 0 : peer.progress.done / peer.progress.total,
  );

  let barVisible = $state(false);
  const gate = new BarGate((v) => (barVisible = v));
  $effect(() => gate.update(syncing));
  onDestroy(() => gate.stop());
</script>

<li class="not-first:border-t not-first:border-line" data-peer={peer.id} data-state={peer.state}>
  <button
    type="button"
    class="block w-full cursor-pointer px-(--pad) py-3 text-left transition-colors duration-(--dur) hover:bg-hover"
    aria-label="{peer.name}, {parts[0]}. Open details"
    onclick={() => deviceSheet.open(peer.id)}
  >
  <div class="flex items-center gap-3">
    <span class="relative grid size-10 shrink-0 place-items-center rounded-md bg-hue-sync-soft text-hue-sync">
      <Icon class="size-5" />
      <span
        class="absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full border-2 border-surface {DOT[peer.state]}"
        aria-hidden="true"
      ></span>
    </span>
    <div class="min-w-0">
      <div class="truncate font-semibold">{peer.name}</div>
      <div class="flex flex-wrap items-center gap-x-1.5 text-sm text-muted">
        <span class="rounded-full px-2 text-xs leading-5 font-semibold {CHIP[peer.state]}">{parts[0]}</span>
        {#if detail}
          <span title={syncing ? undefined : hover}>{detail}</span>
        {/if}
      </div>
    </div>
  </div>
  {#if syncing}
    <ProgressBar
      class="mt-2 ml-[3.25rem]"
      value={fraction}
      visible={barVisible}
      label="Syncing with {peer.name}"
      hue="--hue-sync"
    />
  {/if}
  </button>
</li>
