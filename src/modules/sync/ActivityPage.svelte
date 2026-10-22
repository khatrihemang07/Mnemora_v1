<!-- ActivityPage: a log of what Sync did. -->
<script lang="ts">
  import ArrowLeftRight from '~icons/lucide/arrow-left-right';
  import Card from '../../lib/ui/Card.svelte';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import SyncFrame from './SyncFrame.svelte';
</script>

<SyncFrame tab="activity">
  <Card>
    <EmptyState
      icon={ArrowLeftRight}
      hue="--hue-sync"
      title="No activity yet"
      description="Each time this device syncs with another, it is listed here."
    />
  </Card>
</SyncFrame>
