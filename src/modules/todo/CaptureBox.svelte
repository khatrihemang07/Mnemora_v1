<!--
  CaptureBox: the one box that adds a Task or edits one in place. "+ New task", Q (inline in the
  view, or floating as Quick add anywhere) and Quick edit all use it, so there is one thing to
  learn. As you type it reads dates, weekdays, times, Repeats, #List and p1-p4 (Rust does the
  reading, see `parseCapture`): recognised words are highlighted in the text, and "word → meaning"
  chips below say how each was read. × on a chip, or ⌫ right after the word, keeps it as text, and
  a "kept as text ↺" chip reads it again; a faint "use instead" chip swaps in an earlier word that
  lost to a later one of the same kind. Property chips (List, When, Priority, Notes) set the rest
  by hand; a chip you use turns the typed words of its kind into text, so the two never fight.
  # shows matching Lists (↑ ↓ move, Enter or Tab accept: Enter never adds while they show) and an
  unknown #Name offers "New list" and is made when the Task is added. ⇧↵ drops into notes (⌫ on
  empty notes goes back to the title, ⌘↵ adds). A multi-line paste is one Task: the first line is
  the title, the rest its notes. The titles keep the typed words.
  Adding (mode "add"): Enter adds and the box stays open for the next one; Esc or clicking away
  closes it and drops what was typed. Quick edit (mode "edit"): the box is pre-filled with the
  Task; words already in the old title are not read again, only new text; Enter or clicking away
  saves (with Undo), Esc cancels. `listId` and `date` are what a Task added here inherits.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { onMount, tick } from 'svelte';
  import Calendar from '~icons/lucide/calendar';
  import Clock from '~icons/lucide/clock';
  import Flag from '~icons/lucide/flag';
  import Inbox from '~icons/lucide/inbox';
  import List from '~icons/lucide/list';
  import ListPlus from '~icons/lucide/list-plus';
  import Repeat from '~icons/lucide/repeat';
  import StickyNote from '~icons/lucide/sticky-note';
  import Undo2 from '~icons/lucide/undo-2';
  import X from '~icons/lucide/x';
  import type { Parsed, ParseSpan, Task } from '../../lib/ipc/bindings';
  import { IpcError } from '../../lib/ipc/unwrap';
  import Chip from '../../lib/ui/Chip.svelte';
  import type { WhenValue } from '../../lib/ui/WhenPicker.svelte';
  import Input from '../../lib/ui/Input.svelte';
  import TaskCheckbox from '../../lib/ui/TaskCheckbox.svelte';
  import { notifyError, notifyUndo } from '../../lib/ui/toast';
  import { todoClock } from './clock.svelte';
  import {
    applySuggestion,
    backspaceTarget,
    type CaptureBase,
    type CaptureHand,
    type CaptureWords,
    hashQuery,
    ignoreKinds,
    ignoreWord,
    listSuggestions,
    markSegments,
    MAX_TITLE,
    NO_WORDS,
    pasteParts,
    preferWord,
    pruneWords,
    recogniseAgain,
    resolveFields,
    type Suggestion,
    suggestionKey,
    unchangedRanges,
  } from './capture';
  import { taskRepeat, whenChipLabel } from './dates';
  import ListGlyph from './ListGlyph.svelte';
  import ListPicker from './ListPicker.svelte';
  import PriorityMenu from './PriorityMenu.svelte';
  import { todoPrefs } from './prefs.svelte';
  import {
    createList,
    createTask,
    invalidateTodo,
    listsQuery,
    parseCapture,
    quickEdit,
    quickEditUndo,
  } from './queries';
  import { editedMessage, PRIORITY_NAMES } from './tasks';
  import {
    mergeUnderstood,
    parseUnderstood,
    serializeUnderstood,
    spanMeaning,
    understoodFromSpans,
    type UnderstoodKind,
  } from './understood';
  import WhenHost from './WhenHost.svelte';

  interface Props {
    mode?: 'add' | 'edit';
    /** The Task being edited (mode "edit"). */
    task?: Task | null;
    /** The List a Task added here goes to; none is the Inbox. */
    listId?: string | null;
    /** The day a Task added here is due. */
    date?: string | null;
    placeholder?: string;
    /** Closes the box: Esc, or clicking away when adding. */
    onclose: (reason: 'escape' | 'away') => void;
    /** A Task was added (the box stays open); `where` names its List, or the Inbox. */
    onadded?: (task: Task, where: string) => void;
    /** The quick edit ended: saved, or nothing to save. */
    onsaved?: () => void;
    /** The floating Quick add: its hint line says so. */
    quick?: boolean;
    /** Set while Esc belongs to something inside (suggestions, notes), for a Sheet around. */
    guard?: { escape: boolean };
    class?: string;
  }
  let {
    mode = 'add',
    task = null,
    listId = null,
    date = null,
    placeholder = 'New task',
    quick = false,
    onclose,
    onadded,
    onsaved,
    guard = $bindable({ escape: false }),
    class: cls = '',
  }: Props = $props();

  const editing = $derived(mode === 'edit' && task !== null);
  const PRIORITY_HUE = ['--danger', '--warning', '--accent'];
  const KIND_ICON = { date: Calendar, time: Clock, repeat: Repeat, list: List, prio: Flag } as const;
  const warmChip =
    'bg-[color-mix(in_oklch,var(--hue-todo)_14%,transparent)] text-hue-todo';

  // --- State -------------------------------------------------------------------------------------
  // svelte-ignore state_referenced_locally -- the box starts from the Task it was opened on
  let text = $state(mode === 'edit' && task ? task.title : '');
  let notes = $state('');
  let notesOpen = $state(false);
  let words = $state.raw<CaptureWords>(NO_WORDS);
  let hand = $state.raw<CaptureHand>({});
  let parsed = $state.raw<Parsed | null>(null);
  let parsedFor = $state('');
  let caret = $state(0);
  let suggestIndex = $state(0);
  let dismissedAt = $state<number | null>(null);
  let busy = false;
  let cancelled = false;
  let justUnrecognised = false;

  let root = $state<HTMLElement | null>(null);
  let input = $state<HTMLInputElement | null>(null);
  let mirror = $state<HTMLElement | null>(null);
  let notesField = $state<HTMLTextAreaElement | null>(null);
  let listChip = $state<HTMLButtonElement | null>(null);
  let whenChip = $state<HTMLButtonElement | null>(null);
  let priorityChip = $state<HTMLButtonElement | null>(null);
  let listOpen = $state(false);
  let whenOpen = $state(false);
  let priorityOpen = $state(false);
  const pickerOpen = $derived(listOpen || whenOpen || priorityOpen);

  const sidebar = createQuery(() => listsQuery());
  const lists = $derived((sidebar.data?.lists ?? []).map((e) => e.list));

  const base = $derived<CaptureBase>(
    editing && task
      ? {
          listId: task.list_id,
          date: task.due_date,
          time: task.due_time,
          repeat: taskRepeat(task),
          priority: task.priority,
        }
      : { listId, date, time: null, repeat: null, priority: 4 },
  );
  const fields = $derived(resolveFields(parsed, hand, base));

  // --- Reading the text --------------------------------------------------------------------------
  let sequence = 0;
  $effect(() => {
    const typed = text;
    const decided = words;
    const frozen = editing && task ? unchangedRanges(task.title, typed) : [];
    if (!todoPrefs.parseDates || !typed.trim()) {
      parsed = null;
      parsedFor = typed;
      return;
    }
    const mine = ++sequence;
    parseCapture(typed, decided, frozen)
      .then((p) => {
        if (mine !== sequence) return;
        parsed = p;
        parsedFor = typed;
        // Decisions about words that are gone from the text are forgotten.
        words = pruneWords(decided, [...p.spans, ...p.alternatives, ...p.ignored]);
      })
      .catch(() => {
        if (mine === sequence) parsed = null;
      });
  });

  /** The reading of exactly the text as it is now (waits if the one on screen is behind). */
  async function reading(): Promise<Parsed | null> {
    if (!todoPrefs.parseDates || !text.trim()) return null;
    if (parsedFor === text && parsed) return parsed;
    const frozen = editing && task ? unchangedRanges(task.title, text) : [];
    return parseCapture(text, words, frozen).catch(() => null);
  }

  const meaningCx = $derived({ today: todoClock.today, timeFormat: todoPrefs.timeFormat });
  const mark = (s: ParseSpan) => spanMeaning(s, meaningCx);
  const word = (s: ParseSpan) => text.slice(s.start, s.end);

  // Highlight only while the reading is of this very text, so it never sits on the wrong letters.
  const segments = $derived(
    parsed && parsedFor === text ? markSegments(text, parsed.spans) : [{ text, mark: false }],
  );

  // --- #List suggestions -------------------------------------------------------------------------
  const hash = $derived(hashQuery(text, caret));
  const suggestions = $derived<Suggestion[]>(
    hash && todoPrefs.parseDates && dismissedAt !== hash.start ? listSuggestions(hash.query, lists) : [],
  );
  $effect(() => {
    void hash?.query;
    suggestIndex = 0;
  });
  $effect(() => {
    if (hash === null) dismissedAt = null;
  });
  $effect(() => {
    guard.escape = suggestions.length > 0 || notesOpen;
  });

  function accept(s: Suggestion | undefined) {
    if (!s || !hash) return;
    const next = applySuggestion(text, hash, s);
    text = next.text;
    void tick().then(() => {
      input?.focus();
      input?.setSelectionRange(next.caret, next.caret);
      caret = next.caret;
    });
  }

  // --- Chips -------------------------------------------------------------------------------------
  const listName = (id: string | null) =>
    id === null ? 'Inbox' : (lists.find((l) => l.id === id)?.name ?? 'Inbox');
  const chosenList = $derived(lists.find((l) => l.id === fields.listId) ?? null);
  const whenLabel = $derived(
    whenChipLabel(
      {
        due_date: fields.date,
        due_time: fields.time,
        repeat_freq: fields.repeat?.freq ?? null,
        repeat_every: fields.repeat?.every ?? null,
        repeat_days: fields.repeat?.days ?? null,
        repeat_mode: fields.repeat?.mode ?? null,
      },
      todoClock.today,
      todoPrefs.timeFormat,
    ),
  );

  function keepAsText(key: string) {
    words = ignoreWord(words, key);
    input?.focus();
  }
  function readAgain(key: string) {
    words = recogniseAgain(words, key);
    input?.focus();
  }
  function useInstead(span: ParseSpan) {
    words = preferWord(words, span);
    input?.focus();
  }

  // A chip used by hand is a decision: the typed words of its kinds become text, so the two agree.
  function pickList(list: { id: string } | null) {
    hand = { ...hand, listId: list?.id ?? null };
    words = ignoreKinds(words, parsed, ['list']);
  }
  function pickWhen(next: WhenValue) {
    hand = { ...hand, date: next.date, time: next.time, repeat: next.repeat ?? null };
    words = ignoreKinds(words, parsed, ['date', 'time', 'repeat']);
  }
  function pickPriority(p: number) {
    hand = { ...hand, priority: p };
    words = ignoreKinds(words, parsed, ['prio']);
  }

  // Back to the title once a picker closes.
  let wasOpen = false;
  $effect(() => {
    if (wasOpen && !pickerOpen) void tick().then(() => input?.focus());
    wasOpen = pickerOpen;
  });

  // --- Notes -------------------------------------------------------------------------------------
  async function openNotes() {
    notesOpen = true;
    await tick();
    notesField?.focus();
  }
  function closeNotes() {
    notesOpen = false;
    void tick().then(() => {
      input?.focus();
      input?.setSelectionRange(text.length, text.length);
    });
  }
  function fit(el: HTMLTextAreaElement) {
    el.style.height = 'auto';
    el.style.height = `${el.scrollHeight}px`;
  }
  $effect(() => {
    void notes;
    if (notesField) fit(notesField);
  });

  // --- Adding and saving -------------------------------------------------------------------------
  /** Makes the List a `#Name` asked for, if it is new; returns the List the Task goes to. */
  async function settleList(f: typeof fields): Promise<string | null> {
    if (f.newList === null) return f.listId;
    return (await createList(f.newList)).id;
  }

  /** The record of what the typed words meant, with a List by its id. */
  function record(p: Parsed | null, settled: string | null) {
    return understoodFromSpans(p?.spans ?? [], (s) => (s.kind === 'list' ? settled : s.value));
  }

  async function add() {
    const title = text.trim();
    if (!title || busy) return;
    busy = true;
    const typed = text;
    const typedNotes = notes;
    try {
      const p = await reading();
      const f = resolveFields(p, hand, base);
      const target = await settleList(f);
      const added = await createTask({
        title,
        listId: target,
        date: f.date,
        time: f.time,
        repeat: f.repeat,
        priority: f.priority,
        notes: typedNotes,
        understood: serializeUnderstood(record(p, target)),
      });
      // The next Task starts clean; what was typed meanwhile is kept.
      if (text === typed) {
        text = '';
        caret = 0;
      }
      notes = '';
      notesOpen = false;
      words = NO_WORDS;
      hand = {};
      parsed = null;
      await invalidateTodo();
      onadded?.(added, target === null ? 'Inbox' : (f.newList ?? listName(target)));
      await tick();
      input?.focus();
      input?.scrollIntoView({ block: 'nearest' });
    } catch (e) {
      notifyError(e instanceof IpcError ? e.message : "Couldn't add the Task");
    } finally {
      busy = false;
    }
  }

  async function save() {
    if (!task || busy) return;
    const title = text.trim();
    if (!title) {
      notifyError('A Task needs a title');
      input?.focus();
      return;
    }
    busy = true;
    const old = task;
    try {
      const p = await reading();
      const f = resolveFields(p, hand, base);
      const target = await settleList(f);
      const after = {
        due_date: f.date,
        due_time: f.time,
        repeat_freq: f.repeat?.freq ?? null,
        repeat_every: f.repeat?.every ?? null,
        repeat_days: f.repeat?.days ?? null,
        repeat_mode: f.repeat?.mode ?? null,
        priority: f.priority,
        list_id: target,
      };
      const understood = serializeUnderstood(
        mergeUnderstood(parseUnderstood(old.understood), record(p, target), title, after),
      );
      const previous = {
        title: old.title,
        understood: old.understood,
        due_date: old.due_date,
        due_time: old.due_time,
        repeat: taskRepeat(old),
        priority: old.priority,
        list_id: old.list_id,
      };
      const next = {
        title,
        understood,
        due_date: f.date,
        due_time: f.time,
        repeat: f.repeat,
        priority: f.priority,
        list_id: target,
      };
      if (JSON.stringify(previous) === JSON.stringify(next)) {
        onsaved?.();
        return;
      }
      await quickEdit(old.id, next);
      void invalidateTodo();
      notifyUndo(editedMessage(title), () => {
        quickEditUndo(old.id, previous)
          .catch(() => notifyError("Couldn't undo the edit"))
          .finally(() => void invalidateTodo());
      });
      onsaved?.();
    } catch (e) {
      notifyError(e instanceof IpcError ? e.message : "Couldn't save the Task");
      input?.focus();
    } finally {
      busy = false;
    }
  }

  const submit = () => void (editing ? save() : add());

  function cancel() {
    cancelled = true;
    onclose('escape');
  }

  // --- Keys, paste, focus ------------------------------------------------------------------------
  function syncCaret() {
    caret = input?.selectionStart ?? 0;
    if (mirror && input) mirror.scrollLeft = input.scrollLeft;
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.isComposing) return;
    const key = suggestionKey(e.key, e.shiftKey, {
      count: suggestions.length,
      index: suggestIndex,
    });
    if (key) {
      e.preventDefault();
      e.stopPropagation();
      if (key.type === 'move') suggestIndex = key.index;
      else if (key.type === 'accept') accept(suggestions[suggestIndex]);
      else dismissedAt = hash?.start ?? null;
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      if (e.shiftKey) {
        if (!editing) void openNotes();
      } else submit();
      return;
    }
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      cancel();
      return;
    }
    if (e.key === 'Backspace' && input) {
      const target = backspaceTarget({
        text,
        selectionStart: input.selectionStart ?? 0,
        selectionEnd: input.selectionEnd ?? 0,
        spans: parsed && parsedFor === text ? parsed.spans : [],
        justUnrecognised,
      });
      if (target) {
        e.preventDefault();
        words = ignoreWord(words, target);
        justUnrecognised = true;
        return;
      }
    }
    if (!['Shift', 'Meta', 'Control', 'Alt'].includes(e.key)) justUnrecognised = false;
  }

  function onnoteskeydown(e: KeyboardEvent) {
    if (e.isComposing) return;
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      submit();
    } else if (e.key === 'Escape' || (e.key === 'Backspace' && notes === '')) {
      e.preventDefault();
      e.stopPropagation();
      closeNotes();
    }
  }

  function onpaste(e: ClipboardEvent) {
    const parts = pasteParts(e.clipboardData?.getData('text/plain') ?? '');
    if (!parts || !input) return;
    e.preventDefault();
    if (editing) {
      text = parts.title;
      return;
    }
    const start = input.selectionStart ?? text.length;
    const end = input.selectionEnd ?? text.length;
    text = `${text.slice(0, start)}${parts.title}${text.slice(end)}`.slice(0, MAX_TITLE);
    if (parts.notes) {
      notes = notes ? `${notes}\n${parts.notes}` : parts.notes;
      notesOpen = true;
    }
  }

  // Clicking away: saves a Quick edit, closes an add box. The box's own chips keep the focus where
  // it is (see `keepFocus`), and a picker open from it is part of it.
  function onfocusout(e: FocusEvent) {
    const to = e.relatedTarget;
    if (to instanceof Node && root?.contains(to)) return;
    setTimeout(() => {
      if (cancelled || busy || pickerOpen || !root?.isConnected) return;
      const now = document.activeElement;
      if (now && root.contains(now)) return;
      if (now?.closest('[role="menu"], [role="dialog"], [role="listbox"], [data-editor-popup]')) return;
      if (editing) void save();
      else onclose('away');
    }, 0);
  }

  /** Buttons in the box must not take focus from the title: that would count as clicking away. */
  const keepFocus = (e: Event) => e.preventDefault();

  onMount(() => {
    void tick().then(() => {
      input?.focus();
      input?.setSelectionRange(text.length, text.length);
      caret = text.length;
    });
  });

  const chipBase = 'inline-flex items-center gap-1 rounded-full py-0.5 text-xs';
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -- the group only watches focus leave -->
<div
  bind:this={root}
  role="group"
  aria-label={editing ? 'Quick edit' : 'New task'}
  data-capture-box
  {onfocusout}
  class="rounded-md bg-surface px-2.5 py-2 shadow-pop {cls}"
>
  <div class="grid grid-cols-[26px_1fr] items-center gap-x-2 phone:grid-cols-[30px_1fr]">
    <span aria-hidden="true" class="pointer-events-none grid place-items-center">
      <TaskCheckbox checked={false} label="New task" disabled />
    </span>
    <div class="relative min-w-0">
      <div
        bind:this={mirror}
        aria-hidden="true"
        class="pointer-events-none absolute inset-0 overflow-hidden py-[3px] leading-[1.45] whitespace-pre text-transparent"
      >
        {#each segments as seg, i (i)}{#if seg.mark}<mark
              class="rounded-[4px] bg-[color-mix(in_oklch,var(--hue-todo)_20%,transparent)] text-transparent shadow-[0_0_0_2px_color-mix(in_oklch,var(--hue-todo)_20%,transparent)]"
              >{seg.text}</mark
            >{:else}{seg.text}{/if}{/each}
      </div>
      <Input
        bare
        bind:ref={input}
        bind:value={text}
        label={editing ? 'Task title' : placeholder}
        {placeholder}
        maxlength={MAX_TITLE}
        autocomplete="off"
        spellcheck={false}
        role="combobox"
        aria-expanded={suggestions.length > 0}
        aria-controls="capture-lists"
        aria-activedescendant={suggestions.length > 0 ? `capture-list-${suggestIndex}` : undefined}
        class="relative py-[3px] leading-[1.45]"
        {onkeydown}
        {onpaste}
        oninput={syncCaret}
        onkeyup={syncCaret}
        onclick={syncCaret}
        onselect={syncCaret}
        onscroll={syncCaret}
      />
    </div>
  </div>

  <div class="ml-[34px] phone:ml-[38px]">
    {#if suggestions.length > 0}
      <div
        id="capture-lists"
        role="listbox"
        aria-label="Lists"
        class="my-1 flex w-60 max-w-full flex-col rounded-sm border border-line bg-pop p-1 shadow-pop"
      >
        {#each suggestions as s, i (s.kind === 'list' ? s.id : 'new')}
          <button
            type="button"
            id="capture-list-{i}"
            role="option"
            aria-selected={i === suggestIndex}
            onpointerdown={keepFocus}
            onclick={() => accept(s)}
            class="flex h-8 w-full cursor-pointer items-center gap-2 rounded-sm px-2 text-left text-sm text-ink
              transition-colors duration-(--dur) ease-out hover:bg-hover {i === suggestIndex ? 'bg-hover' : ''}"
          >
            {#if s.kind === 'list'}
              <ListGlyph emoji={s.emoji} />
              <span class="min-w-0 flex-1 truncate">{s.name}</span>
            {:else}
              <ListPlus class="size-4 text-hue-todo" />
              <span class="min-w-0 flex-1 truncate">New list “{s.name}”</span>
            {/if}
          </button>
        {/each}
      </div>
    {/if}

    {#if notesOpen}
      <textarea
        bind:this={notesField}
        bind:value={notes}
        data-bare
        aria-label="Notes"
        placeholder="Notes (Markdown). - [ ] adds a checklist item"
        rows="2"
        onkeydown={onnoteskeydown}
        class="selectable mt-1 block min-h-12 w-full resize-none overflow-hidden border-t border-line bg-transparent pt-1.5 text-[0.9rem]
          leading-[1.5] text-muted outline-none placeholder:text-faint"
      ></textarea>
    {/if}

    {#if parsed && parsedFor === text && (parsed.spans.length > 0 || parsed.alternatives.length > 0 || parsed.ignored.length > 0)}
      <div class="flex flex-wrap items-center gap-1.5 pt-1.5" role="group" aria-label="How the words were read">
        {#each parsed.spans as s (s.key + s.kind)}
          {@const Icon = KIND_ICON[s.kind as UnderstoodKind]}
          <span
            class="{chipBase} {warmChip} pr-1 pl-2"
            title="Understood “{word(s)}” as {mark(s)}"
          >
            <Icon class="size-3" />
            <span class="opacity-70">{word(s)} →</span>
            {mark(s)}
            <button
              type="button"
              aria-label="Keep “{word(s)}” as text"
              title="Keep as text"
              onpointerdown={keepFocus}
              onclick={() => keepAsText(s.key)}
              class="grid size-4 cursor-pointer place-items-center rounded-full transition-colors duration-(--dur) ease-out hover:bg-hover"
            >
              <X class="size-3" />
            </button>
          </span>
        {/each}
        {#each parsed.alternatives as s (s.key + s.kind)}
          {@const Icon = KIND_ICON[s.kind as UnderstoodKind]}
          <button
            type="button"
            title="Another one wins · click to use this one"
            onpointerdown={keepFocus}
            onclick={() => useInstead(s)}
            class="{chipBase} cursor-pointer border border-dashed border-line px-2 text-faint transition-colors duration-(--dur) ease-out hover:border-faint hover:text-muted"
          >
            <Icon class="size-3" />
            {word(s)} → {mark(s)} · use instead
          </button>
        {/each}
        {#each parsed.ignored as s (s.key + s.kind)}
          <button
            type="button"
            title="Click to read it again"
            onpointerdown={keepFocus}
            onclick={() => readAgain(s.key)}
            class="{chipBase} cursor-pointer border border-dashed border-line px-2 text-faint transition-colors duration-(--dur) ease-out hover:border-faint hover:text-muted"
          >
            “{word(s)}” kept as text
            <Undo2 class="size-3" />
          </button>
        {/each}
      </div>
    {/if}

    <div class="mt-1.5 flex flex-wrap items-center gap-1.5 border-t border-dashed border-line pt-1.5">
      <Chip
        bind:ref={listChip}
        tone={chosenList || fields.newList ? 'warm' : 'plain'}
        hue="--hue-todo"
        aria-label="List: {fields.newList ? `new list ${fields.newList}` : (chosenList?.name ?? 'Inbox')}. Choose a List"
        aria-haspopup="dialog"
        onpointerdown={keepFocus}
        onclick={() => (listOpen = true)}
      >
        {#if fields.newList}
          <ListPlus class="size-3.5" />
          <span class="max-w-40 truncate">{fields.newList}</span>
        {:else if chosenList}
          <ListGlyph emoji={chosenList.emoji} class="[&_span]:text-sm [&_svg]:size-3.5" />
          <span class="max-w-40 truncate">{chosenList.name}</span>
        {:else}
          <Inbox class="size-3.5" />
          Inbox
        {/if}
      </Chip>
      <Chip
        bind:ref={whenChip}
        icon={fields.repeat ? Repeat : Calendar}
        tone={fields.date || fields.time ? 'warm' : 'plain'}
        hue="--hue-todo"
        aria-label="When: {whenLabel}"
        aria-haspopup="dialog"
        onpointerdown={keepFocus}
        onclick={() => (whenOpen = true)}
      >
        {whenLabel}
      </Chip>
      <Chip
        bind:ref={priorityChip}
        icon={Flag}
        tone={fields.priority < 4 ? 'warm' : 'plain'}
        hue={PRIORITY_HUE[fields.priority - 1] ?? '--accent'}
        aria-label="Priority: {PRIORITY_NAMES[fields.priority]}"
        aria-haspopup="menu"
        onpointerdown={keepFocus}
        onclick={() => (priorityOpen = true)}
      >
        {fields.priority < 4 ? `P${fields.priority}` : 'Priority'}
      </Chip>
      {#if !editing && !notesOpen}
        <Chip icon={StickyNote} aria-label="Add notes" onpointerdown={keepFocus} onclick={() => void openNotes()}>
          Notes
        </Chip>
      {/if}
    </div>

    <div class="hidden flex-wrap gap-x-3 pt-1.5 text-[0.7rem] text-faint desktop:flex">
      <span>↵ {editing ? 'save' : 'add'}</span>
      {#if !editing}<span>⇧↵ notes</span>{/if}
      <span>Esc {editing ? 'cancel' : 'close'}</span>
      <span>⌫ after a word keeps it as text</span>
      {#if quick}<span class="ml-auto">Quick add · Q</span>{/if}
    </div>
  </div>
</div>

<ListPicker
  bind:open={listOpen}
  anchor={listChip}
  current={fields.listId}
  title="List"
  onpick={pickList}
/>
<PriorityMenu
  bind:open={priorityOpen}
  anchor={priorityChip}
  current={fields.priority}
  returnFocus={null}
  onpick={pickPriority}
/>
<WhenHost
  bind:open={whenOpen}
  anchor={whenChip}
  value={{ date: fields.date, time: fields.time, repeat: fields.repeat }}
  exclude={editing && task ? task.id : null}
  onchange={pickWhen}
/>
