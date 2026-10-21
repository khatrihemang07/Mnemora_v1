<!--
  TaskSheetBody: one Task, open for reading and editing. From the top: the diamond and the title
  (Enter moves into the notes; clearing it and leaving brings the old one back), the chips List ·
  When · Priority, the notes in the live Markdown editor (in the Journal's Writing font), Comments
  with a composer, a History fold (latest line and "Show all"), and a footer with "Created … · Done …"
  and the ⋯ menu. Everything autosaves: title and notes 600 ms after the last keystroke, as one
  editing session per opening, and when the sheet closes. ⌘↑ / ⌘↓ switch to the previous or next Task
  of the list behind it, ⌘↵ and Esc close. A failed save keeps the text and says so.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { onDestroy, onMount, tick, untrack } from 'svelte';
  import CircleX from '~icons/lucide/circle-x';
  import Ellipsis from '~icons/lucide/ellipsis';
  import Flag from '~icons/lucide/flag';
  import Inbox from '~icons/lucide/inbox';
  import SkipForward from '~icons/lucide/skip-forward';
  import Trash2 from '~icons/lucide/trash-2';
  import X from '~icons/lucide/x';
  import type { Task } from '../../lib/ipc/bindings';
  import { queryClient } from '../../lib/ipc/query-client';
  import { IpcError } from '../../lib/ipc/unwrap';
  import { setTimer } from '../../lib/timing';
  import AutoTextarea from '../../lib/ui/AutoTextarea.svelte';
  import Chip from '../../lib/ui/Chip.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import MarkdownInline from '../../lib/ui/MarkdownInline.svelte';
  import MarkdownEditor from '../../lib/ui/MarkdownEditor.svelte';
  import Menu, { type MenuItem } from '../../lib/ui/Menu.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import TaskCheckbox from '../../lib/ui/TaskCheckbox.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import { todoClock } from './clock.svelte';
  import { plainTitle } from './notes';
  import CommentsSection from './CommentsSection.svelte';
  import { isOverdue, stampLabel, taskRepeat, whenChipLabel } from './dates';
  import HistorySection from './HistorySection.svelte';
  import ListGlyph from './ListGlyph.svelte';
  import ListPicker from './ListPicker.svelte';
  import { nextDue } from '../../lib/ui/when-repeat';
  import PriorityMenu from './PriorityMenu.svelte';
  import { todoPrefs } from './prefs.svelte';
  import {
    invalidateTodo,
    listsQuery,
    taskQuery,
    todoKeys,
    updateTask,
  } from './queries';
  import {
    deleteTaskWithUndo,
    moveTaskTo,
    setTaskPriority,
    setTaskWhen,
    skipRepeating,
    tickTask,
    untickTask,
    wontDoTask,
  } from './task-actions';
  import { taskFocus } from './task-focus.svelte';
  import { TaskEditing } from './task-editing';
  import { IDLE_KEYS, reduceKey } from './task-keymap';
  import { inSheetScope } from './task-keys.svelte';
  import { taskSheet } from './task-sheet.svelte';
  import { nextText } from './dates';
  import { priorityTone, PRIORITY_NAMES } from './tasks';
  import { understoodMarks } from './understood';
  import WhenHost from './WhenHost.svelte';

  let { id }: { id: string } = $props();

  const query = createQuery(() => taskQuery(id));
  const sidebar = createQuery(() => listsQuery());
  const stored = $derived(query.data);
  // The words capture understood in the title are underlined while the fields still match them.
  const marks = $derived(
    stored?.understood
      ? understoodMarks(stored, {
          today: todoClock.today,
          timeFormat: todoPrefs.timeFormat,
          listName: (lid) => sidebar.data?.lists.find((e) => e.list.id === lid)?.list.name ?? null,
        })
      : [],
  );
  const list = $derived(
    stored?.list_id ? (sidebar.data?.lists.find((e) => e.list.id === stored.list_id)?.list ?? null) : null,
  );

  // --- Editing: one session per opening of the sheet -----------------------------------------
  let editing = $state.raw<TaskEditing<Task> | null>(null);
  let title = $state('');
  let notes = $state('');
  let saveError = $state<unknown>(null);
  /** The newest `updated_at` this sheet has seen: later ones came from elsewhere (a completion). */
  let knownAt = 0;
  let titleField = $state<HTMLTextAreaElement | null>(null);
  /** The title is shown with its inline Markdown until it is clicked; then it is the raw text. */
  let titleEditing = $state(false);
  const titleLook = 'min-w-0 flex-1 pt-0.5 text-[1.12rem] leading-[1.35] font-semibold';

  async function editTitle() {
    titleEditing = true;
    await tick();
    titleField?.focus();
    titleField?.setSelectionRange(title.length, title.length);
  }
  let editor = $state<ReturnType<typeof MarkdownEditor> | null>(null);

  function begin(t: Task) {
    knownAt = t.updated_at;
    title = t.title;
    notes = t.notes;
    editing = new TaskEditing<Task>(
      {
        update: updateTask,
        setTimer,
        onSaved: (saved) => {
          saveError = null;
          knownAt = Math.max(knownAt, saved.updated_at);
          queryClient.setQueryData(todoKeys.task(id), saved);
          void invalidateTodo();
        },
        onError: (e) => {
          saveError = e;
        },
      },
      t.id,
      { title: t.title, notes: t.notes },
      crypto.randomUUID(),
    );
  }

  $effect(() => {
    const t = stored;
    if (!t) return;
    untrack(() => {
      if (!editing) begin(t);
      // Text changed elsewhere (completing a repeating Task resets its Checklist) is taken as
      // stored, unless there is typing waiting to be saved.
      else if (t.updated_at > knownAt && !editing.unsaved) {
        knownAt = t.updated_at;
        editing.adopt(t);
        title = t.title;
        notes = t.notes;
      }
    });
  });

  // The keys 1 to 4 and ⇧Space act on this Task while the sheet is open.
  $effect(() => {
    taskSheet.task = stored ?? null;
    return () => {
      taskSheet.task = null;
    };
  });

  // A Task that is gone (deleted elsewhere, a stale link): say so and close.
  $effect(() => {
    if (!deleting && query.isError && query.error instanceof IpcError && query.error.code === 'not_found') {
      notifyError('That Task no longer exists');
      taskSheet.close();
    }
  });

  onMount(() => {
    const leaving = () => void editing?.flush();
    const hidden = () => document.visibilityState === 'hidden' && leaving();
    document.addEventListener('visibilitychange', hidden);
    window.addEventListener('pagehide', leaving);
    window.addEventListener('beforeunload', leaving);
    return () => {
      document.removeEventListener('visibilitychange', hidden);
      window.removeEventListener('pagehide', leaving);
      window.removeEventListener('beforeunload', leaving);
    };
  });
  onDestroy(() => void editing?.flush());

  /** The Task as the sheet shows it: stored fields, with the text being edited. */
  const current = $derived<Task | null>(
    stored ? { ...stored, title: title.trim() || stored.title, notes } : null,
  );
  const isOpen = $derived(stored?.state === 'open');
  const overdue = $derived(stored ? isOverdue(stored, todoClock.today) : false);
  const repeat = $derived(stored ? taskRepeat(stored) : null);
  const skipTo = $derived(
    repeat && stored?.due_date ? nextDue(repeat, stored.due_date, todoClock.today) : null,
  );

  async function toggleDone() {
    const t = current;
    if (!t) return;
    await editing?.flush();
    if (t.state === 'open') await tickTask(t);
    else await untickTask(t);
  }

  let deleting = false;

  async function remove() {
    const t = current;
    if (!t) return;
    deleting = true;
    await editing?.flush();
    taskSheet.close();
    void deleteTaskWithUndo(t);
  }

  // --- Chips and menu ---------------------------------------------------------------------------
  let listChip = $state<HTMLButtonElement | null>(null);
  let whenChip = $state<HTMLButtonElement | null>(null);
  let priorityChip = $state<HTMLButtonElement | null>(null);
  let moreButton = $state<HTMLButtonElement | null>(null);
  let listOpen = $state(false);
  let whenOpen = $state(false);
  let priorityOpen = $state(false);
  let menuOpen = $state(false);

  const items = $derived<MenuItem[]>([
    ...(skipTo
      ? [
          {
            label: `Skip to ${nextText(skipTo, todoClock.today)}`,
            icon: SkipForward,
            shortcut: '⇧Space',
            onselect: () => current && void skipRepeating(current),
          },
        ]
      : isOpen
        ? [
            {
              label: 'Won’t do',
              icon: CircleX,
              shortcut: '⇧Space',
              onselect: () => current && void wontDoTask(current),
            },
          ]
        : []),
    {
      label: 'Delete',
      icon: Trash2,
      danger: true,
      separatorBefore: isOpen,
      onselect: () => void remove(),
    },
  ]);

  // --- Keys: ⌘↑ ⌘↓ move through the list behind the sheet, ⌘↵ closes -----------------------------
  /** The Tasks in the list behind the sheet, as they were when it opened. */
  let order: string[] = [];
  const domOrder = () => taskFocus.ids();

  function step(delta: number) {
    if (!order.includes(id)) order = domOrder();
    const at = order.indexOf(id);
    const next = at === -1 ? undefined : order[at + delta];
    if (next) taskSheet.switchTo(next);
  }

  onMount(() => {
    order = domOrder();
  });

  /** The sheet owns ⌘↑ ⌘↓ ⌘↵ even from inside its editor, so it listens before the editor does. */
  function onkeydown(e: KeyboardEvent) {
    if (!inSheetScope(e.target)) return;
    const { action } = reduceKey(
      IDLE_KEYS,
      e,
      { typing: false, overlay: false, sheet: true, target: 'ring', taskOpen: false, onControl: false },
      0,
    );
    if (action?.type === 'sheet-step') step(action.dir);
    else if (action?.type === 'sheet-close') taskSheet.close();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  const footerDone = $derived(
    stored?.closed_at != null && stored.state !== 'open'
      ? `${stored.state === 'wont' ? 'Won’t do' : 'Done'} ${stampLabel(stored.closed_at, todoClock.today, todoPrefs.timeFormat)}`
      : null,
  );
  const PRIORITY_HUE = ['--danger', '--warning', '--accent'];
</script>

<svelte:window onkeydowncapture={onkeydown} />

<div data-task-sheet class="flex flex-col gap-3 px-4 pt-3 pb-3.5 phone:min-h-full phone:px-3.5 phone:pt-6">
  {#if query.isPending || !stored || !editing}
    {#if query.isError}
      <ErrorBanner message="The Task couldn't load." error={query.error} onretry={() => query.refetch()} />
    {:else}
      <div class="flex flex-col gap-3 py-2">
        <Skeleton class="h-6 w-64" />
        <Skeleton class="h-5 w-40" />
        <Skeleton class="h-20 w-full" />
      </div>
    {/if}
  {:else}
    <div class="flex items-start gap-2.5">
      <TaskCheckbox
        checked={!isOpen}
        wont={stored.state === 'wont'}
        label={isOpen ? 'Complete this Task' : 'Reopen this Task'}
        tone={isOpen && stored.priority < 4 ? priorityTone(stored.priority) : overdue ? 'overdue' : 'plain'}
        onchange={() => void toggleDone()}
        class="mt-0.5"
      />
      {#if titleEditing}
        <AutoTextarea
          bind:ref={titleField}
          bind:value={title}
          label="Title"
          maxlength={500}
          placeholder="Title"
          class="{titleLook} {isOpen ? '' : 'text-muted'}"
          oninput={() => (title = editing?.setTitle(title) ?? title)}
          onblur={() => {
            title = editing?.leaveTitle() ?? title;
            titleEditing = false;
          }}
          onkeydown={(e) => {
            if (e.key === 'Enter' && !e.isComposing && !e.metaKey && !e.ctrlKey) editor?.focus();
          }}
        />
      {:else}
        <!-- svelte-ignore a11y_click_events_have_key_events -- Enter and Space are handled below -->
        <div
          role="button"
          tabindex="0"
          aria-label="Title: {plainTitle(title)}. Edit"
          class="{titleLook} cursor-text [overflow-wrap:anywhere] {isOpen ? '' : 'text-muted'}"
          onclick={(e) => !(e.target as Element).closest('a') && void editTitle()}
          onkeydown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              void editTitle();
            }
          }}
        >
          <MarkdownInline source={title} {marks} />
        </div>
      {/if}
      <IconButton icon={X} label="Close" size="sm" onclick={() => taskSheet.close()} class="-mr-1" />
    </div>

    <div class="flex flex-wrap gap-1.5">
      <Chip
        bind:ref={listChip}
        aria-label="List: {list?.name ?? 'Inbox'}. Move to another list"
        aria-haspopup="dialog"
        onclick={() => (listOpen = true)}
      >
        {#if list}<ListGlyph emoji={list.emoji} class="[&_span]:text-sm [&_svg]:size-3.5" />{:else}<Inbox class="size-3.5" />{/if}
        <span class="max-w-40 truncate">{list?.name ?? 'Inbox'}</span>
      </Chip>
      <Chip
        bind:ref={whenChip}
        tone={stored.due_date ? 'warm' : 'plain'}
        hue={overdue ? '--danger' : '--hue-todo'}
        disabled={!isOpen}
        aria-label="When: {whenChipLabel(stored, todoClock.today, todoPrefs.timeFormat)}"
        aria-haspopup="dialog"
        onclick={() => (whenOpen = true)}
      >
        {whenChipLabel(stored, todoClock.today, todoPrefs.timeFormat)}
      </Chip>
      <Chip
        bind:ref={priorityChip}
        icon={Flag}
        tone={stored.priority < 4 ? 'warm' : 'plain'}
        hue={PRIORITY_HUE[stored.priority - 1] ?? '--accent'}
        disabled={!isOpen}
        aria-label="Priority: {PRIORITY_NAMES[stored.priority]}"
        aria-haspopup="menu"
        onclick={() => (priorityOpen = true)}
      >
        {stored.priority < 4 ? `P${stored.priority}` : 'Priority'}
      </Chip>
    </div>

    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -- a bigger target for the editor; it has its own keyboard handling -->
    <div
      class="cursor-text border-t border-line pt-2.5 [--md-cb-hue:var(--hue-todo)]"
      onclick={(e) => e.target === e.currentTarget && editor?.focus()}
    >
      <MarkdownEditor
        bind:this={editor}
        value={notes}
        onchange={(text) => {
          notes = text;
          editing?.setNotes(text);
        }}
        onfinish={() => taskSheet.close()}
        label="Notes"
        placeholder="Notes (Markdown). - [ ] adds a checklist item"
        font={todoPrefs.notesFont}
        bare
        checklist
        class="min-h-20 text-[0.95rem] leading-[1.6]"
      />
    </div>
    {#if saveError !== null}
      <ErrorBanner
        error={saveError}
        message="Couldn't save this Task. Your text is safe here."
        onretry={() => void editing?.flush()}
      />
    {/if}

    <div class="flex flex-col gap-3.5 border-t border-line pt-2.5">
      <CommentsSection taskId={id} />
      <HistorySection taskId={id} />
    </div>

    <div class="flex items-center gap-2.5 border-t border-line pt-2 text-[0.72rem] text-faint phone:mt-auto">
      <span>
        Created {stampLabel(stored.created_at, todoClock.today, todoPrefs.timeFormat, false)}{#if footerDone}{' · '}{footerDone}{/if}
      </span>
      <span class="flex-1"></span>
      <IconButton bind:ref={moreButton} icon={Ellipsis} label="Task actions" size="sm" onclick={() => (menuOpen = true)} />
    </div>
  {/if}
</div>

{#if stored && current}
  <Menu bind:open={menuOpen} anchor={moreButton} {items} label="Task actions" side="top" />
  <ListPicker
    bind:open={listOpen}
    anchor={listChip}
    current={stored.list_id}
    onpick={(picked) => void moveTaskTo(current, picked)}
  />
  <PriorityMenu
    bind:open={priorityOpen}
    anchor={priorityChip}
    current={stored.priority}
    returnFocus={priorityChip}
    onpick={(p) => void setTaskPriority(current, p)}
  />
  {#if isOpen}
    <WhenHost
      bind:open={whenOpen}
      anchor={whenChip}
      value={{ date: stored.due_date, time: stored.due_time, repeat }}
      exclude={stored.id}
      onchange={(next) => void setTaskWhen(current, next.date, next.time, next.repeat ?? null)}
    />
  {/if}
{/if}
