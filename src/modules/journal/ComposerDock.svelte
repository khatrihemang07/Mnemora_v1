<!--
  ComposerDock: the writing box docked at the bottom of the Journal, as wide as the text column.
  The first visible character creates the Entry; later typing autosaves after a pause. Finishing
  (⌘/Ctrl+Enter, Esc or the check) clears the box and the Entry slides into its Page. Until then the
  feed hides it. Clearing the text and finishing (any way, including clicking away or leaving)
  discards the Entry with an Undo; while the box keeps focus, empty text just waits. A failed save
  keeps the text and shows a banner; the next edit retries. The date chip ("Today ▾") puts the Entry
  on any other Page, past or future; it resets to Today when the Entry is finished. ↑ in an empty
  composer hands the keyboard focus to the newest Entry. The box is as wide as the Journal's text column.
-->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import CalendarDays from '~icons/lucide/calendar-days';
  import Check from '~icons/lucide/check';
  import { blockNavigation, interceptBack } from '../../lib/router';
  import { setTimer } from '../../lib/timing';
  import { notifyUndo } from '../../lib/ui/toast';
  import { shortcutLabel } from '../../lib/platform';
  import Chip from '../../lib/ui/Chip.svelte';
  import DatePicker from '../../lib/ui/DatePicker.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import MarkdownEditor from '../../lib/ui/MarkdownEditor.svelte';
  import { viewport } from '../../lib/viewport.svelte';
  import { Composer } from './composer';
  import { chipLabel, localDateString } from './dates';
  import { journalLeaveGuard, useEditing } from './editing.svelte';
  import { useFocus } from './focus.svelte';
  import { useNav } from './navigation.svelte';
  import { useMarkedDays } from './marked-days.svelte';
  import {
    createEntry,
    deleteEntry,
    deviceWrittenWhere,
    invalidateJournal,
    moveEntry,
    restoreEntry,
    updateEntry,
  } from './queries';

  let { class: cls = '' }: { class?: string } = $props();

  const editing = useEditing();
  const nav = useNav();
  const focus = useFocus();
  let text = $state('');
  let error = $state<unknown>(null);
  let focused = $state(false);
  let editor = $state<ReturnType<typeof MarkdownEditor> | null>(null);
  let dateOpen = $state(false);
  let dateChip = $state<HTMLButtonElement | null>(null);
  const pageDate = $derived(nav.composerDate ?? nav.today);

  const composer = new Composer({
    create: async (body, sessionId, chosen) => {
      const now = new Date();
      const entry = await createEntry(
        chosen ?? localDateString(now),
        body,
        deviceWrittenWhere(now),
        sessionId,
      );
      error = null;
      nav.composingId = entry.id;
      return entry;
    },
    update: async (id, body, sessionId) => {
      const entry = await updateEntry(id, body, sessionId);
      error = null;
      return entry;
    },
    move: async (id, date) => {
      const entry = await moveEntry(id, date);
      error = null;
      return entry;
    },
    discard: async (id) => {
      await deleteEntry(id);
      notifyUndo('Entry discarded', () => {
        void restoreEntry(id).then(invalidateJournal);
      });
    },
    setTimer,
    newSessionId: () => crypto.randomUUID(),
    onError: (e) => {
      error = e;
    },
  });

  // The chip is the one source of the date; the composer follows it (and moves an Entry already made).
  $effect(() => composer.setPageDate(nav.composerDate));

  onMount(() => {
    if (!viewport.isPhone) editor?.focus();
    nav.focusComposer = () => editor?.focus();
    nav.compose = () => {
      // Text already written keeps its date; an empty box starts on Today.
      if (!text.trim()) nav.composerDate = null;
      editor?.focus();
    };

    // Hiding (switching apps) only saves: the box may still have focus, so a blank one waits.
    // Closing is a finish, best effort: a blank box discards, anything else saves.
    const hidden = () => {
      if (document.visibilityState === 'hidden') void composer.saveNow();
    };
    const leaving = () => void settle();
    document.addEventListener('visibilitychange', hidden);
    window.addEventListener('pagehide', leaving);
    window.addEventListener('beforeunload', leaving);
    return () => {
      nav.focusComposer = null;
      nav.compose = null;
      document.removeEventListener('visibilitychange', hidden);
      window.removeEventListener('pagehide', leaving);
      window.removeEventListener('beforeunload', leaving);
    };
  });

  // Leaving the Journal mid-sentence still saves it; leaving it cleared discards the Entry.
  onDestroy(() => {
    if (composer.text.trim() || composer.cleared) {
      void composer.finish().then(() => {
        void invalidateJournal().then(() => (nav.composingId = null));
      });
    }
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

  /** Finishes, or discards a cleared Entry. `refocus` is off when focus is leaving on purpose. */
  async function finish(refocus = true) {
    if (!(await composer.finish())) return;
    const id = composer.lastFinishedId;
    text = '';
    error = null;
    nav.composerDate = null;
    if (id) editing.arrived = id;
    await invalidateJournal();
    nav.composingId = null;
    if (refocus && !viewport.isPhone) editor?.focus();
  }

  /** Blur and closing: a cleared Entry is discarded, any other text is only saved. */
  async function settle() {
    if (composer.cleared) await finish(false);
    else await composer.saveNow();
  }

  /** ↑ in an empty box (capture phase, before the editor treats it as a caret move) leaves for the feed. */
  function onkeydowncapture(e: KeyboardEvent) {
    if (e.key !== 'ArrowUp' || text !== '' || e.isComposing) return;
    if (e.metaKey || e.ctrlKey || e.altKey || e.shiftKey) return;
    e.preventDefault();
    e.stopPropagation();
    void focus.fromComposer();
  }

  // The picker's coral dots: days that already have Entries.
  let pickerMonth = $state(nav.today.slice(0, 7));
  const pickerDays = useMarkedDays(() => ({ month: pickerMonth, open: dateOpen }));
</script>

<div class="shrink-0 px-(--pad) pb-(--pad) {cls}">
  <div class="mx-auto max-w-(--journal-width)">
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
      {onkeydowncapture}
    >
      <Chip
        bind:ref={dateChip}
        icon={CalendarDays}
        tone={nav.composerDate === null || nav.composerDate === nav.today ? 'plain' : 'warm'}
        hue="--hue-journal"
        class="mt-0.5 self-start"
        aria-label="Entry date: {chipLabel(pageDate, nav.today)}. Choose another date"
        aria-haspopup="dialog"
        aria-expanded={dateOpen}
        onclick={() => (dateOpen = !dateOpen)}
      >
        {chipLabel(pageDate, nav.today)}
      </Chip>
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
            void settle();
          }
        }}
        label="Write a Journal Entry"
        placeholder="Write…"
        font={editing.font}
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
        onpointerdown={(e) => e.preventDefault()}
        onclick={() => void finish()}
      />
    </div>
  </div>
</div>
<DatePicker
  bind:open={dateOpen}
  anchor={dateChip}
  title="Write on this date"
  today={nav.today}
  weekStart={nav.weekStart}
  value={pageDate}
  marked={pickerDays.marked}
  onmonthchange={(m) => (pickerMonth = m)}
  hue="--hue-journal"
  onpick={(date) => {
    nav.composerDate = date;
    editor?.focus();
  }}
  restoreFocus={() => editor?.focus()}
/>
