<!--
  Branch from earlier: a Dialog listing the answers of the Branch on screen, oldest first. Picking one
  and pressing Branch from here makes the next question continue from it; the rest of the
  Conversation stays, and the pager shows it again.
-->
<script lang="ts">
  import type { EntryView } from '../../lib/ipc/bindings';
  import Button from '../../lib/ui/Button.svelte';
  import Dialog from '../../lib/ui/Dialog.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import { branchFromEarlier } from './branch-actions';
  import { earlierAnswers } from './earlier-answers';

  interface Props {
    open: boolean;
    conversationId: string | null;
    /** The Branch on screen, oldest first. */
    entries: EntryView[];
    onclose: () => void;
    /** Called once the Branch is chosen, so the page can ready the box for the next question. */
    onbranched: () => void;
  }

  let { open, conversationId, entries, onclose, onbranched }: Props = $props();
  let picked = $state<string | null>(null);
  let working = $state(false);
  const rows = $derived(earlierAnswers(entries));

  /** Nothing is chosen each time the picker opens. */
  $effect(() => {
    if (open) picked = null;
  });

  async function branch() {
    if (!conversationId || picked === null || working) return;
    working = true;
    try {
      await branchFromEarlier(conversationId, picked);
      onbranched();
      onclose();
    } catch (error) {
      notifyError(error instanceof Error ? error.message : 'Could not branch from there.');
    } finally {
      working = false;
    }
  }
</script>

<Dialog
  {open}
  title="Branch from earlier"
  description="The next question continues from the answer you pick. The rest of this Conversation stays as it is."
  {onclose}
>
  {#if rows.length === 0}
    <p class="text-sm text-muted">No answers in this Conversation yet.</p>
  {:else}
    <div class="flex max-h-[50vh] flex-col gap-1 overflow-y-auto">
      {#each rows as row (row.answerId)}
        <button
          type="button"
          aria-pressed={picked === row.answerId}
          class="flex w-full min-w-0 flex-col gap-0.5 rounded-sm border px-3 py-2 text-left transition-colors duration-(--dur) ease-out hover:bg-hover {picked === row.answerId
            ? 'border-accent bg-accent-soft'
            : 'border-line'}"
          onclick={() => (picked = row.answerId)}
        >
          <span class="truncate text-sm font-medium text-ink">{row.question}</span>
          <span class="truncate text-sm text-muted">
            {row.start || 'Stopped before any words'}{row.stopped && row.start ? ' · Stopped' : ''}
          </span>
        </button>
      {/each}
    </div>
  {/if}
  {#snippet actions()}
    <Button variant="secondary" onclick={onclose}>Cancel</Button>
    <Button variant="primary" disabled={picked === null || working} onclick={() => void branch()}>
      Branch from here
    </Button>
  {/snippet}
</Dialog>
