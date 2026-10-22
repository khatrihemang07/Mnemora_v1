<!-- SyncOverlay: what Sync mounts while it is on: the "Turn off Sync?" Dialog and, on Android, the Wi-Fi multicast lock mDNS needs. -->
<script lang="ts">
  import { holdMulticastWhileVisible } from './multicast';
  import TurnOffDialog from './TurnOffDialog.svelte';

  $effect(() => holdMulticastWhileVisible(window.MnemoraAndroid));
</script>

<TurnOffDialog />
