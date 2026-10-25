<!-- The question box. Desktop: Enter sends, Shift+Enter adds a line. Phone: Enter adds a line and
     the round button sends. -->
<script lang="ts">
  import ArrowUp from '~icons/lucide/arrow-up';
  import AutoTextarea from '../../lib/ui/AutoTextarea.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import { viewport } from '../../lib/viewport.svelte';

  interface Props {
    /** A Run is answering: the box waits. */
    busy: boolean;
    /** Shown while the box is empty. */
    placeholder: string;
    /** Sends the question. Resolves true when it was accepted, so the box empties only then. */
    onsend: (text: string) => Promise<boolean>;
  }

  let { busy, placeholder, onsend }: Props = $props();
  let draft = $state('');
  const canSend = $derived(draft.trim() !== '' && !busy);

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
  class="mx-auto flex w-full max-w-[42.5rem] items-end gap-2 rounded-md border border-line bg-surface p-2
    transition-colors duration-(--dur) ease-out focus-within:border-accent"
  onsubmit={(event) => {
    event.preventDefault();
    void send();
  }}
>
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
</form>
