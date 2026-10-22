<!-- DevicesPage: the welcome page until the first pairing, then this device and the paired ones. Pairing opens in a Sheet over it. -->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import PairedDevices from './PairedDevices.svelte';
  import PairSheet from './PairSheet.svelte';
  import { syncQuery } from './queries';
  import SyncFrame from './SyncFrame.svelte';
  import WelcomeCard from './WelcomeCard.svelte';

  const sync = createQuery(() => syncQuery);
</script>

<SyncFrame tab="devices">
  {#if sync.isPending}
    <Skeleton class="h-40 w-full" />
  {:else if sync.isError}
    <ErrorBanner error={sync.error} onretry={() => sync.refetch()} />
  {:else if sync.data?.paired_devices === 0}
    <WelcomeCard />
  {:else}
    <PairedDevices />
  {/if}
  <PairSheet />
</SyncFrame>
