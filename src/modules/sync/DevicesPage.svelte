<!-- DevicesPage: the welcome page until the first pairing, then this device and the paired ones. Pairing and a device's details open in a Sheet over it (`?device=‹id›`). -->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import PairedDevices from './PairedDevices.svelte';
  import DeviceSheet from './DeviceSheet.svelte';
  import { deviceSheet } from './device-sheet.svelte';
  import PairSheet from './PairSheet.svelte';
  import { syncQuery } from './queries';
  import SyncFrame from './SyncFrame.svelte';
  import { showWelcome } from './welcome';
  import WelcomeCard from './WelcomeCard.svelte';

  const sync = createQuery(() => syncQuery);

  // This device was forgotten (or left) while a device's sheet was open: nobody is left to show. The
  // sheet is dropped with its URL parameter, not left behind a welcome card (N2).
  $effect(() => {
    if (deviceSheet.id && sync.data?.paired_devices === 0) deviceSheet.dismiss();
  });
</script>

<SyncFrame tab="devices">
  {#if sync.isPending}
    <Skeleton class="h-40 w-full" />
  {:else if sync.isError}
    <ErrorBanner error={sync.error} onretry={() => sync.refetch()} />
  {:else if showWelcome(sync.data?.paired_devices, undefined)}
    <WelcomeCard />
  {:else}
    <PairedDevices />
  {/if}
  <PairSheet />
  {#if deviceSheet.id && sync.data?.paired_devices}
    <DeviceSheet id={deviceSheet.id} />
  {/if}
</SyncFrame>
