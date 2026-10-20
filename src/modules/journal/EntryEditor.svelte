<!--
  EntryEditor: an Entry open for editing in place, inside its row. Autosaves 600ms after the last
  keystroke (and on blur, when the window is hidden or closed, and when leaving), silently. A failed
  save keeps the text and shows a banner; the next edit retries. Finishes on Esc, ⌘/Ctrl+Enter, a
  click outside, Done, or Back on a phone; on desktop the keyboard ways (Esc, ⌘/Ctrl+Enter, Done)
  leave the Entry focused so ↑ ↓ carry on from it. The bottom bar holds a date chip (the Entry's Page; it
  opens the DatePicker to move the Entry, which stays open) and Done. Finishing with all text
  cleared keeps the previous text and says so.
-->
<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from 'svelte';
  import CalendarDays from '~icons/lucide/calendar-days';
  import type { Entry } from '../../lib/ipc/bindings';
  import { blockNavigation, interceptBack } from '../../lib/router';
  import { setTimer } from '../../lib/timing';
  import Button from '../../lib/ui/Button.svelte';
  import Chip from '../../lib/ui/Chip.svelte';
  import DatePicker from '../../lib/ui/DatePicker.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import MarkdownEditor from '../../lib/ui/MarkdownEditor.svelte';
  import { notify } from '../../lib/ui/toast';
  import { viewport } from '../../lib/viewport.svelte';
  import { chipLabel } from './dates';
  import { moveEditedEntry } from './entry-actions';
  import { EntryEditing } from './entry-editing';
  import { journalLeaveGuard, useEditing } from './editing.svelte';
  import { useFocus } from './focus.svelte';
  import { useNav } from './navigation.svelte';
  import { useMarkedDays } from './marked-days.svelte';
  import { invalidateJournal, patchEntry, updateEntry } from './queries';

  let { entry }: { entry: Entry } = $props();

  const editing = useEditing();
  const nav = useNav();
  const focus = useFocus();
  // The session starts from the Entry as it was opened; saves patch the feed copy underneath.
  const start = untrack(() => ({ id: entry.id, body: entry.body }));
  const session = new EntryEditing<Entry>(
    {
      update: updateEntry,
      setTimer,
      onSaved: (saved) => {
        error = null;
        patchEntry(saved);
      },
      onError: (e) => {
        error = e;
      },
    },
    start.id,
    start.body,
    crypto.randomUUID(),
  );

  let text = $state(start.body);
  let error = $state<unknown>(null);
  let card = $state<HTMLDivElement | null>(null);
  let editor = $state<ReturnType<typeof MarkdownEditor> | null>(null);
  let closing: Promise<boolean> | null = null;
  let dateOpen = $state(false);
  let dateChip = $state<HTMLButtonElement | null>(null);

  /** Saves and closes. Stays open, with the banner, when the text could not be saved. */
  function finish(): Promise<boolean> {
    closing ??= (async () => {
      const saved = await session.flush();
      if (saved) {
        if (session.blankKept) notify("Empty entries aren't saved. Kept the previous text.");
        editing.close();
        void invalidateJournal();
      }
      return saved;
    })().finally(() => {
      closing = null;
    });
    return closing;
  }

  /** Finishes from the keyboard or Done: the Entry's row takes the focus back (desktop). */
  async function finishAndFocus() {
    if (!(await finish()) || viewport.isPhone) return;
    await tick();
    await focus.reveal?.(entry.id);
  }

  onMount(() => {
    editing.finisher = finish;
    editor?.focus();

    const outside = (e: PointerEvent) => {
      const target = e.target as Element | null;
      if (!target || card?.contains(target) || target.closest('[data-editor-popup]')) return;
      void finish();
    };
    const hidden = () => {
      if (document.visibilityState === 'hidden') void session.flush();
    };
    const leaving = () => void session.flush();
    document.addEventListener('pointerdown', outside, true);
    document.addEventListener('visibilitychange', hidden);
    window.addEventListener('pagehide', leaving);
    window.addEventListener('beforeunload', leaving);
    return () => {
      document.removeEventListener('pointerdown', outside, true);
      document.removeEventListener('visibilitychange', hidden);
      window.removeEventListener('pagehide', leaving);
      window.removeEventListener('beforeunload', leaving);
    };
  });

  // Leaving the Journal mid-edit still saves.
  onDestroy(() => {
    void session.flush();
    if (editing.finisher === finish) editing.finisher = null;
    if (editing.active === editor) editing.active = null;
  });

  // Text that failed to save is the one thing worth asking about before leaving.
  $effect(() => {
    if (error === null) return;
    return blockNavigation(() => journalLeaveGuard.ask());
  });

  // On a phone, Back finishes the edit instead of leaving the Journal.
  $effect(() => {
    if (!viewport.isPhone) return;
    return interceptBack(() => void finish());
  });

  // The picker's coral dots: days that already have Entries.
  let pickerMonth = $state(nav.today.slice(0, 7));
  const pickerDays = useMarkedDays(() => ({ month: pickerMonth, open: dateOpen }));
</script>

<div bind:this={card} class="min-w-0">
  <MarkdownEditor
    bind:this={editor}
    value={text}
    onchange={(t) => {
      text = t;
      session.input(t);
    }}
    onfinish={() => void finishAndFocus()}
    onfocuschange={(focused) => {
      if (focused) editing.active = editor;
      else {
        if (editing.active === editor) editing.active = null;
        void session.flush();
      }
    }}
    label="Edit this Entry"
    font={editing.font}
    spellcheck={editing.spellcheck}
    slashValues={editing.slashValues}
    bare
  />
  {#if error !== null}
    <!-- A failed save must be seen, even when the Entry runs below the composer. -->
    <div class="mt-3" {@attach (el) => el.scrollIntoView({ block: 'nearest' })}>
      <ErrorBanner
        {error}
        message="Couldn't save this entry. Your text is safe here."
        onretry={() => void session.flush()}
      />
    </div>
  {/if}
  <div class="mt-2 flex items-center justify-between gap-2 border-t border-line pt-2">
    <Chip
      bind:ref={dateChip}
      icon={CalendarDays}
      tone={entry.page_date === nav.today ? 'plain' : 'warm'}
      hue="--hue-journal"
      aria-label="Page date: {chipLabel(entry.page_date, nav.today)}. Move to another date"
      aria-haspopup="dialog"
      aria-expanded={dateOpen}
      onclick={() => (dateOpen = !dateOpen)}
    >
      {chipLabel(entry.page_date, nav.today)}
    </Chip>
    <Button
      size="sm"
      variant="primary"
      onpointerdown={(e) => e.preventDefault()}
      onclick={() => void finishAndFocus()}
    >
      Done
    </Button>
  </div>
</div>
<DatePicker
  bind:open={dateOpen}
  anchor={dateChip}
  title="Move to another date"
  today={nav.today}
  weekStart={nav.weekStart}
  value={entry.page_date}
  marked={pickerDays.marked}
  onmonthchange={(m) => (pickerMonth = m)}
  hue="--hue-journal"
  onpick={(date) => void moveEditedEntry(entry, date, () => session.flush(), nav)}
/>
