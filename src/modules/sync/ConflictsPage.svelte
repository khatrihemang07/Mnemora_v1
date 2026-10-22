<!-- ConflictsPage: where a rewrite of the same text on two devices waits for you. -->
<script lang="ts">
  import GitMerge from '~icons/lucide/git-merge';
  import Card from '../../lib/ui/Card.svelte';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import SyncFrame from './SyncFrame.svelte';
</script>

<SyncFrame tab="conflicts">
  <Card>
    <EmptyState
      icon={GitMerge}
      hue="--hue-sync"
      title="Nothing needs you"
      description="If the same text is rewritten on two devices, both versions wait here for you to choose."
    />
  </Card>
</SyncFrame>
