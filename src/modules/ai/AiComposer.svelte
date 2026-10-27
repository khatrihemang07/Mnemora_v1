<!--
  The question box. Desktop: Enter sends, Shift+Enter adds a line. Phone: Enter adds a line and the
  round button sends. Under the text, the Model pill opens the Model picker; a question needs a Model.
-->
<script lang="ts">
  import ArrowUp from '~icons/lucide/arrow-up';
  import Cpu from '~icons/lucide/cpu';
  import AutoTextarea from '../../lib/ui/AutoTextarea.svelte';
  import Chip from '../../lib/ui/Chip.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import { viewport } from '../../lib/viewport.svelte';
  import type { Choice } from './choice';
  import ModelPicker from './ModelPicker.svelte';

  interface Props {
    /** A Run is answering: the box waits. */
    busy: boolean;
    /** Shown while the box is empty. */
    placeholder: string;
    /** The Model a question is asked with, or none (the pill then says to choose one). */
    choice: Choice | null;
    /** The Model's name for the pill. */
    modelName: string | null;
    /** Sends the question. Resolves true when it was accepted, so the box empties only then. */
    onsend: (text: string) => Promise<boolean>;
    onpick: (choice: Choice) => void;
    /** Whether the Model picker is open; the page can open it (e.g. from a failure banner). */
    pickerOpen?: boolean;
  }

  let { busy, placeholder, choice, modelName, onsend, onpick, pickerOpen = $bindable(false) }: Props =
    $props();
  let draft = $state('');
  let pill = $state<HTMLButtonElement | null>(null);
  const canSend = $derived(draft.trim() !== '' && !busy && choice !== null);

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

<form
  class="mx-auto flex w-full max-w-[42.5rem] flex-col gap-2 rounded-md border border-line bg-surface p-2
    transition-colors duration-(--dur) ease-out focus-within:border-accent"
  onsubmit={(event) => {
    event.preventDefault();
    void send();
  }}
>
  <div class="flex items-end gap-2">
    <AutoTextarea
      bind:value={draft}
      label="Ask a question"
      multiline
      enterBreaks={viewport.isPhone}
      data-bare
      {placeholder}
      class="min-h-6 flex-1 px-2 py-1 text-base"
      {onkeydown}
    />
    <IconButton
      type="submit"
      icon={ArrowUp}
      label="Send"
      variant="solid"
      hue="--hue-ai"
      disabled={!canSend}
    />
  </div>
  <div class="flex items-center gap-2 px-1">
    <Chip bind:ref={pill} icon={Cpu} aria-haspopup="dialog" aria-expanded={pickerOpen} onclick={() => (pickerOpen = !pickerOpen)}>
      {modelName ?? 'Choose a Model'}
    </Chip>
  </div>
</form>

<ModelPicker bind:open={pickerOpen} anchor={pill} {choice} {onpick} />
