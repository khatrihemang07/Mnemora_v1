<!--
  The Saved questions list in AI Settings: add, edit, move up or down, and delete (with Undo). Each
  one runs only when the person taps it in the composer's + menu, so nothing here runs anything.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { tick } from 'svelte';
  import ArrowDown from '~icons/lucide/arrow-down';
  import ArrowUp from '~icons/lucide/arrow-up';
  import Pencil from '~icons/lucide/pencil';
  import Plus from '~icons/lucide/plus';
  import Trash2 from '~icons/lucide/trash-2';
  import type { SavedQuestion } from '../../lib/ipc/bindings';
  import Button from '../../lib/ui/Button.svelte';
  import Card from '../../lib/ui/Card.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import SettingSection from '../../lib/ui/SettingSection.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import SavedQuestionDialog from './SavedQuestionDialog.svelte';
  import { returnFocus } from './focus';
  import { deleteSaved, moveSaved } from './saved-actions';
  import { savedOptions } from './queries';

  const saved = createQuery(() => savedOptions);
  const list = $derived<SavedQuestion[]>(saved.data ?? []);
  let dialogOpen = $state(false);
  let editing = $state<SavedQuestion | null>(null);
  /** The button that opened the dialog: focus goes back to it when the dialog closes. */
  let opener = $state<HTMLElement | null>(null);

  function add(event: MouseEvent) {
    opener = event.currentTarget as HTMLElement;
    editing = null;
    dialogOpen = true;
  }

  function edit(question: SavedQuestion, event: MouseEvent) {
    opener = event.currentTarget as HTMLElement;
    editing = question;
    dialogOpen = true;
  }

  /** The list's rows, so focus can be put back on a control of a row that is still there. */
  let listEl = $state<HTMLElement | null>(null);

  /** A control of the question's row, found by its label; null when the row or control is not there. */
  function controlOf(id: string, label: string): HTMLButtonElement | null {
    return listEl?.querySelector<HTMLButtonElement>(`[data-question="${id}"] [aria-label="${label}"]`) ?? null;
  }

  async function move(question: SavedQuestion, direction: 'up' | 'down') {
    try {
      await moveSaved(list, question.id, direction);
    } catch (failure) {
      notifyError(failure instanceof Error ? failure.message : 'Could not move the question.');
      return;
    }
    // The row moved in the DOM and its control may have lost focus: keep it on the moved row's control.
    await tick();
    const moved = controlOf(question.id, direction === 'up' ? 'Move up' : 'Move down');
    const other = controlOf(question.id, direction === 'up' ? 'Move down' : 'Move up');
    (moved && !moved.disabled ? moved : other && !other.disabled ? other : null)?.focus();
  }

  async function remove(question: SavedQuestion) {
    const index = list.findIndex((candidate) => candidate.id === question.id);
    try {
      await deleteSaved(question);
    } catch (failure) {
      notifyError(failure instanceof Error ? failure.message : 'Could not delete the question.');
      return;
    }
    // The row is gone: focus the Delete of the question that now sits in its place, else the one above.
    await tick();
    const next = list[index] ?? list[index - 1];
    (next ? controlOf(next.id, 'Delete') : null)?.focus();
  }
</script>

<SettingSection
  title="Saved questions"
  description="One-tap questions in the composer's + menu. They run only when you tap them."
>
  {#if saved.isError}
    <ErrorBanner error={saved.error} onretry={() => void saved.refetch()} />
  {:else}
    <Card padded={false}>
      <div bind:this={listEl} class="contents">
      {#each list as question, index (question.id)}
        <div data-question={question.id} class="flex items-center gap-2 border-b border-line py-2 pr-2 pl-(--pad) first:rounded-t-md">
          <span class="min-w-0 flex-1 truncate text-ink">{question.text}</span>
          <IconButton icon={ArrowUp} label="Move up" size="sm" variant="ghost" disabled={index === 0} onclick={() => void move(question, 'up')} />
          <IconButton icon={ArrowDown} label="Move down" size="sm" variant="ghost" disabled={index === list.length - 1} onclick={() => void move(question, 'down')} />
          <IconButton icon={Pencil} label="Edit" size="sm" variant="ghost" onclick={(event) => edit(question, event)} />
          <IconButton icon={Trash2} label="Delete" size="sm" variant="ghost" onclick={() => void remove(question)} />
        </div>
      {:else}
        <p class="px-(--pad) py-3 text-sm text-muted">No Saved questions yet. Save one here, or from an answer.</p>
      {/each}
      </div>
      <div class="flex justify-start p-(--pad)">
        <Button variant="secondary" onclick={(event: MouseEvent) => add(event)}><Plus class="size-4" /> Save a question</Button>
      </div>
    </Card>
  {/if}
</SettingSection>

<SavedQuestionDialog
  open={dialogOpen}
  saved={editing}
  onclose={() => (dialogOpen = false)}
  restoreFocus={() => returnFocus(opener)}
/>
