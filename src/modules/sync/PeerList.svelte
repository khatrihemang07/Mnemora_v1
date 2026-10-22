<!-- PeerList: the Paired devices list. Simple rows for now (icon, name, when paired); status, counts and the device sheet come with their own ticket. -->
<script lang="ts">
  import Laptop from '~icons/lucide/laptop';
  import Smartphone from '~icons/lucide/smartphone';
  import type { PeerRow } from '../../lib/ipc/bindings';
  import Card from '../../lib/ui/Card.svelte';

  let { peers }: { peers: PeerRow[] } = $props();

  const day = (ms: number) =>
    new Date(ms).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' });
</script>

<h2 class="mt-4 mb-1.5 text-xs font-medium text-faint">Paired devices</h2>
<Card padded={false}>
  <ul>
    {#each peers as peer (peer.id)}
      {@const Icon = peer.platform === 'phone' ? Smartphone : Laptop}
      <li class="flex items-center gap-3 px-(--pad) py-3 not-first:border-t not-first:border-line">
        <span class="grid size-10 shrink-0 place-items-center rounded-md bg-hue-sync-soft text-hue-sync"><Icon class="size-5" /></span>
        <div class="min-w-0">
          <div class="truncate font-semibold">{peer.name}</div>
          <div class="text-sm text-muted">Paired {day(peer.paired_at)}</div>
        </div>
      </li>
    {/each}
  </ul>
</Card>
