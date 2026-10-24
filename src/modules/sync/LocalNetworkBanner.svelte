<!-- LocalNetworkBanner: the system keeps this device off the local network (macOS Local Network privacy, Android 17's local network permission; cards P14 and P16). On Devices and in the Pair sheet, with the one button that goes to the switch. Nothing is drawn while all is fine. -->
<script lang="ts">
  import ShieldAlert from '~icons/lucide/shield-alert';
  import type { ThisDevice } from '../../lib/ipc/bindings';
  import Button from '../../lib/ui/Button.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import { localNetworkBanner, openLocalNetworkSettings } from './local-network';

  let {
    device,
    class: className = '',
  }: { device: Pick<ThisDevice, 'platform' | 'local_network_blocked'> | undefined; class?: string } =
    $props();

  const banner = $derived(localNetworkBanner(device));

  async function open() {
    if (!device) return;
    try {
      await openLocalNetworkSettings(device.platform);
    } catch (e) {
      notifyError(e instanceof Error ? e.message : 'Could not open Settings');
    }
  }
</script>

{#if banner}
  <div
    role="alert"
    data-local-network-blocked
    class="flex flex-wrap items-start gap-2.5 rounded-md border border-danger bg-danger-soft px-3.5 py-2.5 text-left text-sm text-ink {className}"
  >
    <ShieldAlert class="mt-0.5 size-4 shrink-0 text-danger" />
    <div class="min-w-0 flex-1">
      <b class="font-semibold">{banner.title}</b><br />
      <span class="text-muted">{banner.body}</span>
    </div>
    <Button size="sm" onclick={open}>{banner.button}</Button>
  </div>
{/if}
