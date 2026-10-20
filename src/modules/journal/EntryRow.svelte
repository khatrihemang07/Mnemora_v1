<!--
  EntryRow: one Entry on a Page. Desktop puts the time in a 64px left margin, phone puts it above the
  text. When it was written on another day than its Page, a short date comes with the time (stacked
  above it on desktop). Hovering the time shows the full written timestamp and zone. The text is
  rendered Markdown; clicking it (not a link or a checklist row) opens it for editing in place on a
  lifted card while the other Entries dim. A checklist row ticks on click instead. A ⋯ button (on
  hover or focus on desktop, always on a phone), right-click and long-press open the actions Menu
  (Edit, Move to another date…, Copy text, Delete). Move opens the DatePicker; a flash tints the
  row when the Entry was just moved here or found through the move toast's Show. On desktop the row
  is also a stop for the keyboard focus ring (↑ ↓ move between Entries across Pages, Enter edits,
  ⌫ / Delete deletes with Undo, M moves, C copies, Esc returns to the composer).
-->
<script lang="ts">
  import CalendarDays from '~icons/lucide/calendar-days';
  import Copy from '~icons/lucide/copy';
  import Ellipsis from '~icons/lucide/ellipsis';
  import PenLine from '~icons/lucide/pen-line';
  import Trash2 from '~icons/lucide/trash-2';
  import type { Entry } from '../../lib/ipc/bindings';
  import { tick } from 'svelte';
  import { LongPress } from '../../lib/long-press';
  import type { TimeFormat } from '../../lib/time/format';
  import DatePicker from '../../lib/ui/DatePicker.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import Menu, { type MenuAnchor, type MenuItem } from '../../lib/ui/Menu.svelte';
  import MarkdownView from '../../lib/ui/MarkdownView.svelte';
  import { viewport } from '../../lib/viewport.svelte';
  import { editedLabel, fullWritten, writtenDateLabel, writtenTime } from './dates';
  import { copyEntryText, deleteEntryWithUndo, moveEntryToDate, tickTask } from './entry-actions';
  import { useEditing } from './editing.svelte';
  import EntryEditor from './EntryEditor.svelte';
  import { useFocus } from './focus.svelte';
  import { useNav } from './navigation.svelte';
  import { useMarkedDays } from './marked-days.svelte';

  let { entry, timeFormat }: { entry: Entry; timeFormat: TimeFormat } = $props();

  const editing = useEditing();
  const nav = useNav();
  const focus = useFocus();
  const open = $derived(editing.id === entry.id);
  const dimmed = $derived(editing.id !== null && !open);

  const time = $derived(writtenTime(entry.written_at, entry.written_offset_min, timeFormat));
  const writtenDate = $derived(
    writtenDateLabel(entry.written_at, entry.written_offset_min, entry.page_date),
  );
  const full = $derived(
    fullWritten(entry.written_at, entry.written_offset_min, entry.written_tz, timeFormat),
  );
  // The edit time is shown on the viewer's own clock.
  const edited = $derived(
    entry.edited_at === null
      ? null
      : editedLabel(entry.edited_at, -new Date(entry.edited_at).getTimezoneOffset(), timeFormat),
  );

  let row = $state<HTMLElement | null>(null);
  let menuOpen = $state(false);
  let menuAnchor = $state.raw<MenuAnchor | null>(null);
  let moveOpen = $state(false);
  let moveAnchor = $state.raw<HTMLElement | null>(null);

  /** Opens the editor; the Entry keeps the keyboard focus. */
  function openEditor() {
    focus.send({ type: 'edit', id: entry.id });
    void editing.open(entry.id);
  }

  /** Deletes with Undo, then the ring moves to the next Entry (or the composer when none is left). */
  async function deleteFocused() {
    focus.send({ type: 'deleted', id: entry.id });
    const next = focus.id;
    const id = entry.id;
    // Undo puts the ring back on the restored Entry, so the keys carry on from where they were.
    await deleteEntryWithUndo(entry, editing, () => {
      focus.send({ type: 'focus', id });
      void focus.reveal?.(id);
    });
    await tick();
    if (next) await focus.reveal?.(next);
    else nav.focusComposer?.();
  }

  // The keys are shown as hints beside the actions, but only where there is a keyboard to use them.
  const hint = (key: string) => (viewport.isPhone ? undefined : key);
  const actions: MenuItem[] = $derived([
    { label: 'Edit', icon: PenLine, shortcut: hint('↵'), onselect: openEditor },
    {
      label: 'Move to another date…',
      icon: CalendarDays,
      shortcut: hint('M'),
      // After the menu has closed, so its dismissal cannot count as an outside click on the picker.
      onselect: () => {
        moveAnchor = menuAnchor instanceof HTMLElement ? menuAnchor : row;
        setTimeout(() => (moveOpen = true), 0);
      },
    },
    { label: 'Copy text', icon: Copy, shortcut: hint('C'), onselect: () => void copyEntryText(entry) },
    {
      label: 'Delete',
      icon: Trash2,
      shortcut: hint('⌫'),
      danger: true,
      separatorBefore: true,
      onselect: () => void deleteFocused(),
    },
  ]);

  function openMenu(anchor: MenuAnchor) {
    if (open) return;
    menuAnchor = anchor;
    menuOpen = true;
  }

  /** Right-click on desktop, and the long-press Android turns into a context menu. */
  function oncontextmenu(e: MouseEvent) {
    if (open) return; // the open editor keeps the native menu (spelling, paste)
    e.preventDefault();
    // The keyboard's menu key reports no position: hang it off the row.
    openMenu(e.clientX === 0 && e.clientY === 0 && row ? row : { x: e.clientX, y: e.clientY });
  }

  // Where the platform has no context menu for touch, a long-press stands in for it.
  const press = new LongPress({ onpress: (x, y) => openMenu({ x, y }) });
  const touchLike = (e: PointerEvent) => e.pointerType === 'touch' || e.pointerType === 'pen';

  /** Links and checkboxes keep their own click; so does selecting text with the mouse. */
  function onrowclick(e: MouseEvent) {
    if (press.fired) {
      // The lift after a long-press must not also open the editor.
      press.fired = false;
      return;
    }
    if (open || (e.target as Element).closest('a, button, [role="checkbox"]')) return;
    if (window.getSelection()?.toString()) return;
    openEditor();
  }

  /** Keys on the row itself (not on the editor, checkboxes or ⋯ inside it). No ⌘⌫: macOS owns it. */
  function onrowkeydown(e: KeyboardEvent) {
    if (open || e.target !== e.currentTarget || e.metaKey || e.ctrlKey || e.altKey) return;
    switch (e.key) {
      case 'Enter':
        openEditor();
        break;
      case 'ArrowUp':
      case 'ArrowDown':
        void focus.move(e.key === 'ArrowUp' ? 'up' : 'down');
        break;
      case 'Backspace':
      case 'Delete':
        void deleteFocused();
        break;
      case 'm':
      case 'M':
        moveAnchor = row;
        moveOpen = true;
        break;
      case 'c':
      case 'C':
        void copyEntryText(entry);
        break;
      case 'Escape':
        if (viewport.isPhone) return;
        nav.focusComposer?.();
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  // The picker's coral dots: days that already have Entries.
  let pickerMonth = $state(nav.today.slice(0, 7));
  const pickerDays = useMarkedDays(() => ({ month: pickerMonth, open: moveOpen }));
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -- the row is the click-to-edit target; Enter on it opens the editor from the keyboard -->
<article
  bind:this={row}
  tabindex="0"
  aria-label="Entry at {time}"
  aria-keyshortcuts={viewport.isPhone ? undefined : 'ArrowUp ArrowDown Enter Backspace Delete M C'}
  class="group relative -mx-2.5 grid scroll-mt-20 scroll-mb-8 gap-0.5 rounded-md px-2.5 py-2 transition-[opacity,background-color,box-shadow] duration-(--dur) ease-out desktop:grid-cols-[4rem_1fr] desktop:gap-x-3.5 [&+&]:before:absolute [&+&]:before:inset-x-2.5 [&+&]:before:top-0 [&+&]:before:h-px [&+&]:before:bg-line
    {open ? 'bg-surface shadow-pop' : 'cursor-text hover:bg-hover focus-visible:bg-hover'} {dimmed ? 'opacity-35' : ''}
    {editing.arrived === entry.id ? 'slide-in' : ''} {nav.flashEntry === entry.id ? 'flash' : ''}"
  data-entry-id={entry.id}
  onclick={onrowclick}
  onkeydown={onrowkeydown}
  onfocus={() => focus.send({ type: 'focus', id: entry.id })}
  {oncontextmenu}
  onpointerdown={(e) => touchLike(e) && !open && press.start(e.clientX, e.clientY)}
  onpointermove={(e) => touchLike(e) && press.move(e.clientX, e.clientY)}
  onpointerup={() => press.cancel()}
  onpointercancel={() => press.cancel()}
  onanimationend={() => {
    if (editing.arrived === entry.id) editing.arrived = null;
  }}
>
  <time class="text-xs text-faint tabular-nums desktop:pt-[3px] desktop:text-[0.78rem]" title={full}>
    {#if writtenDate}
      <span class="mr-1.5 desktop:mr-0 desktop:block desktop:text-[0.65rem] desktop:leading-[1.3] desktop:whitespace-nowrap">
        {writtenDate}
      </span>
    {/if}
    {time}
  </time>
  <div class="min-w-0">
    {#if open}
      <EntryEditor {entry} />
    {:else}
      <MarkdownView
        source={entry.body}
        class="break-words"
        font={editing.font}
        ontoggle={(line) => void tickTask(entry, line)}
      />
      {#if edited}
        <p class="mt-0.5 text-[0.72rem] text-faint" title={edited}>edited</p>
      {/if}
    {/if}
  </div>
  {#if !open}
    <!-- Hover or focus reveals it on desktop; a phone has no hover, so it stays. Solid, because it
         sits over the end of the first line. -->
    <IconButton
      icon={Ellipsis}
      label="Entry actions"
      size="sm"
      class="absolute top-1 right-1.5 bg-surface-2 transition-opacity desktop:opacity-0 desktop:group-focus-within:opacity-100 desktop:group-hover:opacity-100
        {menuOpen ? 'desktop:opacity-100' : ''}"
      aria-haspopup="menu"
      aria-expanded={menuOpen}
      onclick={(e) => openMenu(e.currentTarget)}
    />
  {/if}
</article>
<Menu bind:open={menuOpen} anchor={menuAnchor} items={actions} label="Entry actions" returnFocus={row} />
<DatePicker
  bind:open={moveOpen}
  anchor={moveAnchor}
  title="Move to another date"
  today={nav.today}
  weekStart={nav.weekStart}
  value={entry.page_date}
  marked={pickerDays.marked}
  onmonthchange={(m) => (pickerMonth = m)}
  hue="--hue-journal"
  onpick={(date) => void moveEntryToDate(entry, date, editing, nav)}
  restoreFocus={() => row?.focus()}
/>
