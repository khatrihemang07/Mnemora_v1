<!--
  ComposerDock: the writing box docked at the bottom of the Journal, as wide as the text column.
  The first visible character creates the Entry; later typing autosaves after a pause. Finishing
  (⌘/Ctrl+Enter, Esc or the check) clears the box and shows the Entry in the feed.
-->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import Check from '~icons/lucide/check';
  import { hasMod, shortcutLabel } from '../../lib/platform';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import Textarea from '../../lib/ui/Textarea.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import { viewport } from '../../lib/viewport.svelte';
  import { Composer } from './composer';
  import { localDateString } from './dates';
  import { createEntry, deviceWrittenWhere, invalidateJournal, updateEntry } from './queries';

  let { class: cls = '' }: { class?: string } = $props();

  let text = $state('');
  let el = $state<HTMLTextAreaElement | null>(null);

  const composer = new Composer({
    create: (body, sessionId) => {
      const now = new Date();
      return createEntry(localDateString(now), body, deviceWrittenWhere(now), sessionId);
    },
    update: updateEntry,
    setTimer: (fn, ms) => {
      const t = setTimeout(fn, ms);
      return () => clearTimeout(t);
    },
    newSessionId: () => crypto.randomUUID(),
    onError: () => notifyError("Couldn't save your Entry. Your text is still here."),
  });

  onMount(() => {
    if (!viewport.isPhone) el?.focus();
  });

  // Leaving the Journal mid-sentence still saves it.
  onDestroy(() => {
    if (composer.text.trim()) void composer.finish().then(invalidateJournal);
  });

  async function finish() {
    if (!(await composer.finish())) return;
    text = '';
    await invalidateJournal();
    if (!viewport.isPhone) el?.focus();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' || (e.key === 'Enter' && hasMod(e))) {
      e.preventDefault();
      void finish();
    }
  }
</script>

<div class="shrink-0 px-(--pad) pb-(--pad) {cls}">
  <div
    class="mx-auto flex max-w-[42.5rem] items-end gap-2 rounded-md border border-line bg-surface p-2 shadow-card
      transition-colors duration-(--dur) ease-out focus-within:border-accent"
  >
    <Textarea
      bind:value={text}
      bind:el
      label="Write a Journal Entry"
      placeholder="Write…"
      bare
      class="max-h-48 min-h-8 px-1 py-1 font-display text-base leading-[1.65]"
      oninput={(e) => composer.input(e.currentTarget.value)}
      {onkeydown}
    />
    <IconButton
      icon={Check}
      label="Finish Entry"
      shortcut={shortcutLabel('↵')}
      tooltipSide="top"
      onclick={finish}
    />
  </div>
</div>
