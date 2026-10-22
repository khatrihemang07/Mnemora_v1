<!-- DevicesPage: the welcome page until the first pairing, then this device and the paired ones. Pairing opens in a Sheet over it. -->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import PairSheet from './PairSheet.svelte';
  import PeerList from './PeerList.svelte';
  import { devicesQuery, syncQuery } from './queries';
  import SyncFrame from './SyncFrame.svelte';
  import ThisDeviceCard from './ThisDeviceCard.svelte';
  import WelcomeCard from './WelcomeCard.svelte';

  const sync = createQuery(() => syncQuery);
  const devices = createQuery(() => devicesQuery);
</script>

<SyncFrame tab="devices">
  {#if sync.isPending || (sync.data?.paired_devices !== 0 && devices.isPending)}
    <Skeleton class="h-40 w-full" />
  {:else if sync.isError}
    <ErrorBanner error={sync.error} onretry={() => sync.refetch()} />
  {:else if devices.isError}
    <ErrorBanner error={devices.error} onretry={() => devices.refetch()} />
  {:else if sync.data?.paired_devices === 0}
    <WelcomeCard />
  {:else if devices.data}
    <ThisDeviceCard device={devices.data.this_device} />
    <PeerList peers={devices.data.peers} />
  {/if}
  <PairSheet />
</SyncFrame>
