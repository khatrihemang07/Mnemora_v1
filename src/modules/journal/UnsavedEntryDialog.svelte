<!-- UnsavedEntryDialog: asked before leaving the Journal with text that could not be saved. -->
<script lang="ts">
  import Button from '../../lib/ui/Button.svelte';
  import Dialog from '../../lib/ui/Dialog.svelte';
  import { journalLeaveGuard } from './editing.svelte';
</script>

<Dialog
  open={journalLeaveGuard.open}
  title="Leave without saving?"
  description="This entry couldn't be saved yet. If you leave now, your latest text is lost."
  onclose={() => journalLeaveGuard.answer(false)}
>
  {#snippet actions()}
    <Button variant="ghost" onclick={() => journalLeaveGuard.answer(true)}>Leave</Button>
    <Button variant="primary" onclick={() => journalLeaveGuard.answer(false)}>Stay</Button>
  {/snippet}
</Dialog>
