<!--
  SyncFrame: the chrome every Sync page shares: the page title with "Sync now", and the Devices ·
  Conflicts · Activity tabs under it. Each tab is a route, so choosing one navigates.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import type { Snippet } from 'svelte';
  import RadioTower from '~icons/lucide/radio-tower';
  import RefreshCw from '~icons/lucide/refresh-cw';
  import Button from '../../lib/ui/Button.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import Tabs from '../../lib/ui/Tabs.svelte';
  import { moduleContext } from '../../shell/module-context';
  import { syncQuery } from './queries';
  import { pathForTab, SYNC_TABS, type SyncTab } from './tabs';

  let { tab, children }: { tab: SyncTab; children: Snippet } = $props();

  const sync = createQuery(() => syncQuery);
  // Nothing to sync with until the first pairing.
  const alone = $derived((sync.data?.paired_devices ?? 0) === 0);
</script>

<div data-sync class="selectable h-full overflow-y-auto p-(--pad)">
  <PageHeader title="Sync" icon={RadioTower} hue="--hue-sync">
    {#snippet actions()}
      <Button variant="ghost" size="sm" disabled={alone} aria-label="Sync now">
        <RefreshCw class="size-3.5" />
        <span class="phone:sr-only">Sync now</span>
      </Button>
    {/snippet}
  </PageHeader>
  <div class="mx-auto max-w-3xl">
    <Tabs
      value={tab}
      tabs={SYNC_TABS}
      label="Sync pages"
      hue="--hue-sync"
      onHue="--on-hue-sync"
      onchange={(t) => moduleContext.navigate(pathForTab(t))}
    >
      {@render children()}
    </Tabs>
  </div>
</div>
