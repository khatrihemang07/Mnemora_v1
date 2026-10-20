<!-- UnsavedDialog: Save / Discard / Cancel when navigating away from unsaved Settings. -->
<script lang="ts">
  import Button from '../../lib/ui/Button.svelte';
  import Dialog from '../../lib/ui/Dialog.svelte';
  import { leaveGuard } from './leave-guard.svelte';
  import { save } from './save.svelte';

  async function saveAndLeave() {
    leaveGuard.answer(await save.run());
  }
  function discardAndLeave() {
    save.discard();
    leaveGuard.answer(true);
  }
</script>

<Dialog
  open={leaveGuard.open}
  title="Unsaved changes"
  description="You changed some Settings but haven't saved them."
  onclose={() => leaveGuard.answer(false)}
>
  {#snippet actions()}
    <Button variant="ghost" onclick={() => leaveGuard.answer(false)}>Cancel</Button>
    <Button variant="secondary" onclick={discardAndLeave}>Discard</Button>
    <Button variant="primary" onclick={saveAndLeave}>Save</Button>
  {/snippet}
</Dialog>
