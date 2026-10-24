<!-- TurnOffDialog: "Turn off Sync?", asked from Settings › Modules and Settings › Sync. -->
<script lang="ts">
  import Button from '../../lib/ui/Button.svelte';
  import Dialog from '../../lib/ui/Dialog.svelte';
  import { confirmTurnOff, turnOffDialog } from './switch.svelte';
</script>

<Dialog
  open={turnOffDialog.open}
  title="Turn off Sync?"
  description="This device stops syncing. Your data and paired devices are kept; turn Sync on again to catch up."
  onclose={() => (turnOffDialog.open = false)}
>
  {#snippet actions()}
    <Button onclick={() => (turnOffDialog.open = false)}>Cancel</Button>
    <Button variant="primary" onclick={() => void confirmTurnOff()}>Turn off</Button>
  {/snippet}
</Dialog>
