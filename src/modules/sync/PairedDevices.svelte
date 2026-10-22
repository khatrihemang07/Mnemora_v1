<!-- PairedDevices: this device and the paired ones. Its own query lives here so that it is read in every state this component renders (a query read only on some branches misses updates to the rest). -->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import PeerList from './PeerList.svelte';
  import { devicesQuery, watchDevices } from './queries';
  import ThisDeviceCard from './ThisDeviceCard.svelte';

  const devices = createQuery(() => devicesQuery);

  // Status and progress arrive as events while this is on screen.
  $effect(() => {
    let stop: (() => void) | undefined;
    let gone = false;
    void watchDevices().then((off) => (gone ? off() : (stop = off)));
    return () => {
      gone = true;
      stop?.();
    };
  });
</script>

{#if devices.isError}
  <ErrorBanner error={devices.error} onretry={() => devices.refetch()} />
{:else if devices.data}
  <ThisDeviceCard device={devices.data.this_device} />
  <PeerList peers={devices.data.peers} />
{:else}
  <Skeleton class="h-40 w-full" />
{/if}
