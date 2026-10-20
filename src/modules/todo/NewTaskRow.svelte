<!--
  NewTaskRow: "+ New task" at the end of the list. Click (or Enter) turns it into an inline field in
  place. Enter adds the Task and keeps the field open for the next; Esc or clicking away closes it
  (what was typed is dropped). Rust validates the title; a rejected one stays in the field with the
  reason in a toast.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import Plus from '~icons/lucide/plus';
  import { IpcError } from '../../lib/ipc/unwrap';
  import Input from '../../lib/ui/Input.svelte';
  import TaskCheckbox from '../../lib/ui/TaskCheckbox.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import { createTask, invalidateTodo } from './queries';

  /** Longest title; Rust enforces it too. */
  const MAX_TITLE = 500;

  let open = $state(false);
  let value = $state('');
  let input = $state<HTMLInputElement | null>(null);
  let opener = $state<HTMLButtonElement | null>(null);

  async function show() {
    open = true;
    await tick();
    input?.focus();
  }

  function close(returnFocus: boolean) {
    open = false;
    value = '';
    if (returnFocus) void tick().then(() => opener?.focus());
  }

  async function add() {
    const title = value;
    if (!title.trim()) return;
    value = '';
    try {
      await createTask(title);
    } catch (e) {
      // Put the text back unless the user already started the next one.
      if (!value) value = title;
      notifyError(e instanceof IpcError ? e.message : "Couldn't add the Task");
      return;
    }
    await invalidateTodo();
    await tick();
    input?.scrollIntoView({ block: 'nearest' });
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.isComposing) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      void add();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close(true);
    }
  }
</script>

{#if open}
  <div
    class="-mx-0.5 my-1.5 grid grid-cols-[26px_1fr] items-center gap-x-2 rounded-md bg-surface px-2.5 py-2 shadow-pop
      phone:grid-cols-[30px_1fr]"
  >
    <span aria-hidden="true" class="pointer-events-none grid place-items-center">
      <TaskCheckbox checked={false} label="New task" disabled />
    </span>
    <Input
      bare
      bind:ref={input}
      bind:value
      label="New task"
      placeholder="New task"
      maxlength={MAX_TITLE}
      autocomplete="off"
      class="py-[3px] leading-[1.45]"
      {onkeydown}
      onblur={() => close(false)}
    />
  </div>
{:else}
  <button
    bind:this={opener}
    type="button"
    onclick={show}
    class="grid w-full cursor-text grid-cols-[26px_1fr] items-center gap-x-2 rounded-sm px-2 py-[7px] text-left text-faint
      transition-colors duration-(--dur) ease-out hover:bg-hover hover:text-muted phone:grid-cols-[30px_1fr] phone:px-1.5 phone:py-2.5"
  >
    <Plus class="size-4 justify-self-center" />
    <span>New task</span>
  </button>
{/if}
