<!-- Adds or edits one Saved question. Saving closes it; a failure stays on screen with its reason. -->
<script lang="ts">
  import type { SavedQuestion } from '../../lib/ipc/bindings';
  import { IpcError } from '../../lib/ipc/unwrap';
  import Button from '../../lib/ui/Button.svelte';
  import Dialog from '../../lib/ui/Dialog.svelte';
  import Input from '../../lib/ui/Input.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import { renameSaved, saveQuestion } from './saved-actions';

  interface Props {
    open: boolean;
    /** The question being edited, or none to add one. */
    saved: SavedQuestion | null;
    onclose: () => void;
  }

  let { open, saved, onclose }: Props = $props();
  let text = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);

  // Starts from the question being edited each time the dialog opens.
  $effect(() => {
    if (open) {
      text = saved?.text ?? '';
      error = null;
    }
  });

  // Rust checks the text (blank, over 500 characters) and its reason shows under the field; only
  // a failure that is not about the text gets a toast.
  async function save() {
    busy = true;
    error = null;
    try {
      if (saved) await renameSaved(saved.id, text);
      else await saveQuestion(text);
      onclose();
    } catch (failure) {
      error = failure instanceof Error ? failure.message : 'Could not save the question.';
      if (!(failure instanceof IpcError && failure.code === 'validation')) notifyError(error);
    } finally {
      busy = false;
    }
  }
</script>

<Dialog
  {open}
  title={saved ? 'Edit Saved question' : 'Save a question'}
  description="It runs with one tap from the + menu, and only then."
  {onclose}
>
  <form
    class="flex flex-col gap-2"
    onsubmit={(event) => {
      event.preventDefault();
      void save();
    }}
  >
    <Input bind:value={text} label="Question" placeholder="Summarise my last 7 days" invalid={error !== null} />
    {#if error}<p role="alert" class="text-sm text-danger">{error}</p>{/if}
  </form>
  {#snippet actions()}
    <Button variant="secondary" onclick={onclose}>Cancel</Button>
    <Button variant="primary" disabled={busy} onclick={() => void save()}>
      {saved ? 'Save' : 'Save a question'}
    </Button>
  {/snippet}
</Dialog>
