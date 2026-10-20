<!--
  ComposerDock: the writing box docked at the bottom of the Journal, as wide as the text column.
  The first visible character creates the Entry; later typing autosaves after a pause. Finishing
  (⌘/Ctrl+Enter, Esc or the check) clears the box and the Entry slides into its Page. A failed save
  keeps the text and shows a banner; the next edit retries.
-->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import Check from '~icons/lucide/check';
  import { blockNavigation, interceptBack } from '../../lib/router';
  import { shortcutLabel } from '../../lib/platform';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import MarkdownEditor from '../../lib/ui/MarkdownEditor.svelte';
  import { viewport } from '../../lib/viewport.svelte';
  import { Composer } from './composer';
  import { localDateString } from './dates';
  import { journalLeaveGuard, useEditing } from './editing.svelte';
  import { createEntry, deviceWrittenWhere, invalidateJournal, updateEntry } from './queries';

  let { class: cls = '' }: { class?: string } = $props();

  const editing = useEditing();
  let text = $state('');
  let error = $state<unknown>(null);
  let focused = $state(false);
  let editor = $state<ReturnType<typeof MarkdownEditor> | null>(null);

  const composer = new Composer({
    create: async (body, sessionId) => {
      const now = new Date();
      const entry = await createEntry(
        localDateString(now),
        body,
        deviceWrittenWhere(now),
        sessionId,
      );
      error = null;
      return entry;
    },
    update: async (id, body, sessionId) => {
      const entry = await updateEntry(id, body, sessionId);
      error = null;
      return entry;
    },
    setTimer: (fn, ms) => {
      const t = setTimeout(fn, ms);
      return () => clearTimeout(t);
    },
    newSessionId: () => crypto.randomUUID(),
    onError: (e) => {
      error = e;
    },
  });

  onMount(() => {
    if (!viewport.isPhone) editor?.focus();

    const hidden = () => {
      if (document.visibilityState === 'hidden') void composer.saveNow();
    };
    const leaving = () => void composer.saveNow();
    document.addEventListener('visibilitychange', hidden);
    window.addEventListener('pagehide', leaving);
    window.addEventListener('beforeunload', leaving);
    return () => {
      document.removeEventListener('visibilitychange', hidden);
      window.removeEventListener('pagehide', leaving);
      window.removeEventListener('beforeunload', leaving);
    };
  });

  // Leaving the Journal mid-sentence still saves it.
  onDestroy(() => {
    if (composer.text.trim()) void composer.finish().then(invalidateJournal);
    if (editing.active === editor) editing.active = null;
  });

  // Text that failed to save is the one thing worth asking about before leaving.
  $effect(() => {
    if (error === null || !text.trim()) return;
    return blockNavigation(() => journalLeaveGuard.ask());
  });

  // On a phone, Back finishes what you are writing instead of leaving the Journal.
  $effect(() => {
    if (!viewport.isPhone || !focused || !text.trim()) return;
    return interceptBack(() => void finish());
  });

  async function finish() {
    if (!(await composer.finish())) return;
    const id = composer.lastFinishedId;
    text = '';
    error = null;
    if (id) editing.arrived = id;
    await invalidateJournal();
    if (!viewport.isPhone) editor?.focus();
  }
</script>

<div class="shrink-0 px-(--pad) pb-(--pad) {cls}">
  <div class="mx-auto max-w-[42.5rem]">
    {#if error !== null}
      <div class="mb-2">
        <ErrorBanner
          {error}
          message="Couldn't save this entry. Your text is safe here."
          onretry={() => void composer.saveNow()}
        />
      </div>
    {/if}
    <div
      class="flex items-end gap-2 rounded-md border border-line bg-surface p-2 shadow-card
        transition-colors duration-(--dur) ease-out focus-within:border-accent"
    >
      <MarkdownEditor
        bind:this={editor}
        value={text}
        onchange={(t) => {
          text = t;
          editing.arrived = null;
          composer.input(t);
        }}
        onfinish={() => void finish()}
        onfocuschange={(f) => {
          focused = f;
          if (f) editing.active = editor;
          else {
            if (editing.active === editor) editing.active = null;
            void composer.saveNow();
          }
        }}
        label="Write a Journal Entry"
        placeholder="Write…"
        spellcheck={editing.spellcheck}
        slashValues={editing.slashValues}
        maxHeight="12rem"
        bare
        class="min-h-8 flex-1 px-1 py-1"
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
</div>
