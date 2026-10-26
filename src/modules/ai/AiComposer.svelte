<!--
  The question box, as the approved prototype has it: the text on top, then a row of round tools
  (+ for Saved questions, the Model chip) and the round send at the right. While a Run answers the
  round button is Stop, until something is typed: then it sends the typed text as a note for the
  Run. Desktop: Enter sends, Shift+Enter adds a line. Phone: Enter adds a line and the round
  button sends.
-->
<script lang="ts">
  import Bookmark from '~icons/lucide/bookmark';
  import ArrowUp from '~icons/lucide/arrow-up';
  import Cpu from '~icons/lucide/cpu';
  import Plus from '~icons/lucide/plus';
  import Settings from '~icons/lucide/settings';
  import Square from '~icons/lucide/square';
  import type { SavedQuestion } from '../../lib/ipc/bindings';
  import AutoTextarea from '../../lib/ui/AutoTextarea.svelte';
  import Button from '../../lib/ui/Button.svelte';
  import Chip from '../../lib/ui/Chip.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import type { MenuItem } from '../../lib/ui/Menu.svelte';
  import { viewport } from '../../lib/viewport.svelte';
  import ActionMenu from './ActionMenu.svelte';
  import type { Choice } from './choice';
  import ModelPicker from './ModelPicker.svelte';

  interface Props {
    /** A Run is answering: the round button stops it until something is typed, which queues a note. */
    busy: boolean;
    /** Shown while the box is empty. */
    placeholder: string;
    /** The Model a question is asked with, or none (the pill then says to choose one). */
    choice: Choice | null;
    /** The Model's name for the pill. */
    modelName: string | null;
    /** The Saved questions, for the + menu. */
    saved: SavedQuestion[];
    /** Sends the question, or while a Run answers, queues the text as a note. Resolves true when it
     *  was accepted, so the box empties only then. */
    onsend: (text: string) => Promise<boolean>;
    /** Stops the Run that is answering. */
    onstop: () => void;
    /** Runs a Saved question (one tap). */
    onrunsaved: (text: string) => void;
    /** Opens the AI Settings where Saved questions are kept. */
    onmanage: () => void;
    /** Edit: the question being changed. Its text fills the box, and sending makes an edited sibling. */
    editing: { text: string } | null;
    /** Cancel Edit: the box goes back to asking a new question. */
    oncancelEdit: () => void;
    onpick: (choice: Choice) => void;
    /** Whether the Model picker is open; the page can open it (e.g. from a failure banner). */
    pickerOpen?: boolean;
    /** The question box, so the page can focus it on request. */
    ref?: HTMLTextAreaElement | null;
  }

  let {
    busy,
    placeholder,
    choice,
    modelName,
    saved,
    onsend,
    onstop,
    onrunsaved,
    onmanage,
    editing,
    oncancelEdit,
    onpick,
    pickerOpen = $bindable(false),
    ref = $bindable(null),
  }: Props = $props();
  let draft = $state('');
  /** Each Edit fills the box with its question's text (a new object each time, so the effect runs). */
  $effect(() => {
    if (editing) draft = editing.text;
  });
  let pill = $state<HTMLButtonElement | null>(null);
  let plus = $state<HTMLButtonElement | null>(null);
  let plusOpen = $state(false);
  const canSend = $derived(draft.trim() !== '' && choice !== null);
  /** Stop only while the box is empty, so a note can still be sent from a phone (Enter adds a line there). */
  const stopping = $derived(busy && draft.trim() === '');

  /** The + menu: each Saved question, then the way to manage them. */
  const plusItems = $derived<MenuItem[]>([
    ...(saved.length === 0
      ? [{ label: 'No Saved questions yet', icon: Bookmark, disabled: true, onselect: () => {} }]
      : saved.map((question) => ({
          label: question.text,
          icon: Bookmark,
          onselect: () => onrunsaved(question.text),
        }))),
    {
      label: 'Manage in AI Settings',
      icon: Settings,
      separatorBefore: true,
      onselect: onmanage,
    },
  ]);

  function cancelEdit() {
    draft = '';
    oncancelEdit();
  }

  async function send() {
    if (!canSend) return;
    const text = draft;
    if (await onsend(text)) draft = '';
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key !== 'Enter' || event.shiftKey || event.isComposing || viewport.isPhone) return;
    event.preventDefault();
    void send();
  }
</script>

{#if editing}
  <div class="mx-auto flex w-full max-w-[42.5rem] items-center justify-between gap-2 px-2 pb-1">
    <span class="text-xs text-muted">Editing a question</span>
    <Button size="sm" variant="ghost" onclick={cancelEdit}>Cancel</Button>
  </div>
{/if}
<form
  class="mx-auto flex w-full max-w-[42.5rem] flex-col gap-2 rounded-md border border-line bg-surface p-2
    transition-colors duration-(--dur) ease-out focus-within:border-accent"
  onsubmit={(event) => {
    event.preventDefault();
    void send();
  }}
>
  <AutoTextarea
    bind:value={draft}
    bind:ref
    label="Ask a question"
    multiline
    enterBreaks={viewport.isPhone}
    data-bare
    {placeholder}
    class="min-h-6 px-2 py-1 text-base"
    {onkeydown}
  />
  <div class="flex items-center gap-2">
    <IconButton
      bind:ref={plus}
      icon={Plus}
      label="Saved questions"
      variant="secondary"
      size="sm"
      aria-haspopup="menu"
      aria-expanded={plusOpen}
      onclick={() => (plusOpen = !plusOpen)}
    />
    <Chip bind:ref={pill} icon={Cpu} aria-haspopup="dialog" aria-expanded={pickerOpen} onclick={() => (pickerOpen = !pickerOpen)}>
      {modelName ?? 'Choose a Model'}
    </Chip>
    <span class="flex-1"></span>
    {#if stopping}
      <IconButton icon={Square} label="Stop" variant="solid" hue="--ink" onclick={onstop} />
    {:else}
      <IconButton
        type="submit"
        icon={ArrowUp}
        label={busy ? 'Add a note' : 'Send'}
        variant="solid"
        hue="--hue-ai"
        disabled={!canSend}
      />
    {/if}
  </div>
</form>
{#if !viewport.isPhone}
  <p class="px-1 pt-1 text-xs text-muted">Enter sends · Shift+Enter new line</p>
{/if}

<ActionMenu bind:open={plusOpen} anchor={plus} items={plusItems} label="Saved questions" side="top" />
<ModelPicker bind:open={pickerOpen} anchor={pill} {choice} {onpick} />
