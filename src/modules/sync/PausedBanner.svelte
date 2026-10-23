<!-- PausedBanner: why sync with a device is paused and what fixes it (cards D8–D10 "Devices (paused)"). One per paused device. Both devices keep working on their own. -->
<script lang="ts">
  import Pause from '~icons/lucide/pause';
  import type { PeerRow } from '../../lib/ipc/bindings';
  import { pauseBanner } from './pause';

  let { peers }: { peers: PeerRow[] } = $props();

  const banners = $derived(
    peers.flatMap((peer) => {
      const banner = pauseBanner(peer);
      return banner === null ? [] : [{ id: peer.id, ...banner }];
    }),
  );
</script>

{#each banners as banner (banner.id)}
  <div
    role="status"
    data-paused={banner.id}
    class="mb-3 flex items-start gap-2.5 rounded-md border border-[color-mix(in_oklch,var(--warning)_50%,transparent)] bg-[color-mix(in_oklch,var(--warning)_16%,transparent)] px-3.5 py-2.5 text-sm text-ink"
  >
    <Pause class="mt-0.5 size-4 shrink-0 text-warning" />
    <div>
      <div class="font-semibold">{banner.title}</div>
      <div class="text-muted">{banner.text} Both devices keep working on their own.</div>
    </div>
  </div>
{/each}
