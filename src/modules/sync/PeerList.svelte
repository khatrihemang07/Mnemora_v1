<!-- PeerList: the Paired devices list, reachable devices first (the order comes from Rust). -->
<script lang="ts">
  import type { PeerRow as Peer } from '../../lib/ipc/bindings';
  import Card from '../../lib/ui/Card.svelte';
  import PeerRow from './PeerRow.svelte';

  let { peers }: { peers: Peer[] } = $props();
</script>

<h2 class="mt-4 mb-1.5 text-xs font-medium text-faint">Paired devices</h2>
<Card padded={false}>
  <ul>
    {#each peers as peer (peer.id)}
      <PeerRow {peer} />
    {/each}
  </ul>
</Card>
