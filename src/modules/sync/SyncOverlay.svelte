<!-- SyncOverlay: what Sync mounts while it is on: the "Turn off Sync?" Dialog, on Android the Wi-Fi multicast lock mDNS needs, and the nudge that makes unreachable devices be tried again when the app or the network comes back. -->
<script lang="ts">
  import { commands } from '../../lib/ipc/bindings';
  import { holdMulticastWhileVisible } from './multicast';
  import { resumeWhenBack } from './resume';
  import TurnOffDialog from './TurnOffDialog.svelte';

  $effect(() => holdMulticastWhileVisible(window.MnemoraAndroid));
  $effect(() => resumeWhenBack(() => void commands.syncResume()));
</script>

<TurnOffDialog />
