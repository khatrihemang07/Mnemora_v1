<!-- SyncOverlay: what Sync mounts while it is on: the "Turn off Sync?" Dialog, the toast for new conflicts (when this device is set to one), what keeps the badge and the Home card fresh, on Android the Wi-Fi multicast lock mDNS needs, and the nudge that makes unreachable devices be tried again when the app or the network comes back. -->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { commands, events } from '../../lib/ipc/bindings';
  import { notifyAction } from '../../lib/ui/toast';
  import { moduleContext } from '../../shell/module-context';
  import { newConflictsText } from './attention';
  import { syncAttention } from './hint.svelte';
  import { attentionQuery, watchAttention, whileMounted } from './queries';
  import { holdMulticastWhileVisible } from './multicast';
  import { resumeWhenBack } from './resume';
  import TurnOffDialog from './TurnOffDialog.svelte';

  $effect(() => holdMulticastWhileVisible(window.MnemoraAndroid));
  $effect(() => resumeWhenBack(() => void commands.syncResume()));
  $effect(() => whileMounted(watchAttention));
  const attention = createQuery(() => attentionQuery);
  $effect(() => {
    syncAttention.current = attention.data;
    return () => {
      syncAttention.current = undefined;
    };
  });
  // Rust only sends this to a device set to "Show a toast".
  $effect(() =>
    whileMounted(() =>
      events.syncNewConflictsEvent.listen((e) =>
        notifyAction(newConflictsText(e.payload.count, e.payload.device), 'Review', () =>
          moduleContext.navigate('/sync/conflicts'),
        ),
      ),
    ),
  );
</script>

<TurnOffDialog />
