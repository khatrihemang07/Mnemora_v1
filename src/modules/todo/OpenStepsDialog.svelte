<!--
  OpenStepsDialog: "3 steps are still open". Completing a Task whose notes hold unticked Checklist
  items asks every time (one of the two deliberate confirmations): Cancel, "Tick all and complete"
  (the notes get every step ticked first) or Complete (the steps stay as they are). Todo's page
  mounts it once; `stepsPrompt` says which Task is being completed.
-->
<script lang="ts">
  import Button from '../../lib/ui/Button.svelte';
  import Dialog from '../../lib/ui/Dialog.svelte';
  import { openSteps, plainTitle } from './notes';
  import { stepsPrompt } from './steps-prompt.svelte';
  import { completeNow, tickAllAndComplete } from './task-actions';

  const task = $derived(stepsPrompt.task);
  // Kept while the Dialog fades, so its words do not change as it closes.
  let shown = $state.raw(stepsPrompt.task);
  $effect(() => {
    if (task) shown = task;
  });
  const count = $derived(shown ? openSteps(shown.notes) : 0);

  function answer(run: (() => Promise<unknown>) | null) {
    const target = shown;
    stepsPrompt.close();
    if (run && target) void run();
  }
</script>

<Dialog
  open={task !== null}
  title="{count} {count === 1 ? 'step is' : 'steps are'} still open"
  description={shown ? `“${plainTitle(shown.title)}” has unticked Checklist items.` : ''}
  onclose={() => answer(null)}
>
  {#snippet actions()}
    <Button onclick={() => answer(null)}>Cancel</Button>
    <Button onclick={() => answer(() => (shown ? tickAllAndComplete(shown) : Promise.resolve()))}>
      Tick all and complete
    </Button>
    <Button variant="primary" onclick={() => answer(() => (shown ? completeNow(shown) : Promise.resolve()))}>
      Complete
    </Button>
  {/snippet}
</Dialog>
