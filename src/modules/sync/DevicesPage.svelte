<!-- DevicesPage: the welcome page until the first pairing, then this device and the paired ones. -->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import Laptop from '~icons/lucide/laptop';
  import Plus from '~icons/lucide/plus';
  import Smartphone from '~icons/lucide/smartphone';
  import Button from '../../lib/ui/Button.svelte';
  import Card from '../../lib/ui/Card.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import { notify } from '../../lib/ui/toast';
  import { viewport } from '../../lib/viewport.svelte';
  import { syncQuery } from './queries';
  import SyncFrame from './SyncFrame.svelte';
  import WelcomeCard from './WelcomeCard.svelte';

  const sync = createQuery(() => syncQuery);
  const Icon = $derived(viewport.isPhone ? Smartphone : Laptop);
</script>

<SyncFrame tab="devices">
  {#if sync.isPending}
    <Skeleton class="h-40 w-full" />
  {:else if sync.isError}
    <ErrorBanner error={sync.error} onretry={() => sync.refetch()} />
  {:else if sync.data.paired_devices === 0}
    <WelcomeCard />
  {:else}
    <Card>
      <div class="flex items-center gap-3">
        <span class="grid size-10 shrink-0 place-items-center rounded-md bg-hue-sync-soft text-hue-sync">
          <Icon class="size-5" />
        </span>
        <div class="min-w-0 flex-1">
          <div class="truncate font-semibold">{sync.data.name}</div>
          <div class="text-sm text-muted">
            This device · paired with {sync.data.paired_devices}
            {sync.data.paired_devices === 1 ? 'device' : 'devices'}
          </div>
        </div>
        <Button variant="primary" onclick={() => notify('Pairing is not available yet.')}>
          <Plus class="size-4" /> Pair a device
        </Button>
      </div>
    </Card>
  {/if}
</SyncFrame>
