<!--
  CommentsSection: a Task's Comments in the sheet, oldest first, with a composer under them.
  Comments take inline Markdown marks (bold, italic, code, links). Hover (always on a phone) shows
  edit and delete; Enter in the edit field saves, Esc cancels, an edited comment says "edited".
  Deleting is at once with Undo. Comments belong to the current occurrence of a repeating Task.
  The list reloads from Rust after every change.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { tick } from 'svelte';
  import MessageSquare from '~icons/lucide/message-square';
  import Pencil from '~icons/lucide/pencil';
  import Trash2 from '~icons/lucide/trash-2';
  import type { Comment } from '../../lib/ipc/bindings';
  import { IpcError } from '../../lib/ipc/unwrap';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import Input from '../../lib/ui/Input.svelte';
  import MarkdownInline from '../../lib/ui/MarkdownInline.svelte';
  import { notifyError, notifyUndo } from '../../lib/ui/toast';
  import { stampLabel } from './dates';
  import { todoClock } from './clock.svelte';
  import { todoPrefs } from './prefs.svelte';
  import {
    addComment,
    commentsQuery,
    deleteComment,
    editComment,
    invalidateTodo,
    restoreComment,
  } from './queries';

  let { taskId }: { taskId: string } = $props();

  const comments = createQuery(() => commentsQuery(taskId));
  const list = $derived(comments.data ?? []);

  let draft = $state('');
  let editingId = $state<string | null>(null);
  let editText = $state('');
  let editField = $state<HTMLInputElement | null>(null);

  const reason = (e: unknown, fallback: string) => (e instanceof IpcError ? e.message : fallback);

  async function post() {
    const body = draft.trim();
    if (!body) return;
    try {
      await addComment(taskId, body);
      draft = '';
    } catch (e) {
      notifyError(reason(e, "Couldn't add the comment"));
    }
    void invalidateTodo();
  }

  async function startEdit(c: Comment) {
    editingId = c.id;
    editText = c.body;
    await tick();
    editField?.focus();
  }

  async function saveEdit() {
    const id = editingId;
    if (id === null) return;
    editingId = null;
    const body = editText.trim();
    const was = list.find((c) => c.id === id);
    if (!body || body === was?.body) return;
    try {
      await editComment(id, body);
    } catch (e) {
      notifyError(reason(e, "Couldn't save the comment"));
    }
    void invalidateTodo();
  }

  async function remove(c: Comment) {
    try {
      await deleteComment(c.id);
    } catch {
      notifyError("Couldn't delete the comment");
      return;
    } finally {
      void invalidateTodo();
    }
    notifyUndo('Comment deleted', () => {
      restoreComment(c.id)
        .catch(() => notifyError("Couldn't restore the comment"))
        .finally(() => void invalidateTodo());
    });
  }

  const reveal = 'opacity-0 group-hover:opacity-100 focus-visible:opacity-100 phone:opacity-100';
</script>

<section aria-label="Comments" class="flex flex-col gap-1.5">
  <h3 class="flex items-center gap-1.5 text-[0.69rem] font-semibold tracking-[0.07em] text-faint uppercase">
    <MessageSquare class="size-3.5" />Comments{#if list.length > 0}<span class="tabular-nums">{list.length}</span>{/if}
  </h3>
  {#each list as c (c.id)}
    <div class="group grid grid-cols-[1fr_auto] gap-x-2 rounded-sm bg-surface-2 px-2.5 py-1.5 text-[0.84rem] leading-normal">
      {#if editingId === c.id}
        <Input
          bind:ref={editField}
          bind:value={editText}
          label="Edit comment"
          maxlength={1000}
          class="col-span-2"
          onkeydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              void saveEdit();
            } else if (e.key === 'Escape') {
              e.preventDefault();
              e.stopPropagation();
              editingId = null;
            }
          }}
          onblur={() => void saveEdit()}
        />
      {:else}
        <div class="min-w-0 [overflow-wrap:anywhere]"><MarkdownInline source={c.body} /></div>
        <div class="row-span-2 col-start-2 row-start-1 flex items-start gap-0.5">
          <IconButton icon={Pencil} label="Edit comment" size="xs" onclick={() => void startEdit(c)} class="{reveal} -mt-0.5" />
          <IconButton icon={Trash2} label="Delete comment" size="xs" onclick={() => void remove(c)} class="{reveal} -mt-0.5" />
        </div>
        <small class="text-[0.72rem] text-faint">
          {stampLabel(c.created_at, todoClock.today, todoPrefs.timeFormat)}{c.edited ? ' · edited' : ''}
        </small>
      {/if}
    </div>
  {/each}
  <Input
    bind:value={draft}
    label="Add a comment"
    placeholder="Add a comment…  (↵ to post)"
    maxlength={1000}
    onkeydown={(e) => {
      if (e.key === 'Enter' && !e.isComposing) {
        e.preventDefault();
        void post();
      }
    }}
  />
</section>
