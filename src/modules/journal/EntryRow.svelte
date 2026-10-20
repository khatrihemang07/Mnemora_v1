<!--
  EntryRow: one Entry on a Page. Desktop puts the time in a 64px left margin, phone puts it above the
  text. When it was written on another day than its Page, a short date comes with the time (stacked
  above it on desktop). Hovering the time shows the full written timestamp and zone. The text is
  rendered Markdown; clicking it (not a link or a checklist row) opens it for editing in place on a
  lifted card while the other Entries dim. A checklist row ticks on click instead. A ⋯ button (on
  hover or focus on desktop, always on a phone), right-click and long-press open the actions Menu.
-->
<script lang="ts">
  import Copy from '~icons/lucide/copy';
  import Ellipsis from '~icons/lucide/ellipsis';
  import PenLine from '~icons/lucide/pen-line';
  import Trash2 from '~icons/lucide/trash-2';
  import type { Entry } from '../../lib/ipc/bindings';
  import { LongPress } from '../../lib/long-press';
  import type { TimeFormat } from '../../lib/time/format';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import Menu, { type MenuAnchor, type MenuItem } from '../../lib/ui/Menu.svelte';
  import MarkdownView from '../../lib/ui/MarkdownView.svelte';
  import { editedLabel, fullWritten, writtenDateLabel, writtenTime } from './dates';
  import { copyEntryText, deleteEntryWithUndo, tickTask } from './entry-actions';
  import { useEditing } from './editing.svelte';
  import EntryEditor from './EntryEditor.svelte';

  let { entry, timeFormat }: { entry: Entry; timeFormat: TimeFormat } = $props();

  const editing = useEditing();
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

  const actions: MenuItem[] = [
    { label: 'Edit', icon: PenLine, onselect: () => void editing.open(entry.id) },
    { label: 'Copy text', icon: Copy, onselect: () => void copyEntryText(entry) },
    {
      label: 'Delete',
      icon: Trash2,
      danger: true,
      separatorBefore: true,
      onselect: () => void deleteEntryWithUndo(entry, editing),
    },
  ];

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
    void editing.open(entry.id);
  }
  function onrowkeydown(e: KeyboardEvent) {
    if (open || e.target !== e.currentTarget || e.key !== 'Enter') return;
    e.preventDefault();
    void editing.open(entry.id);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -- the row is the click-to-edit target; Enter on it opens the editor from the keyboard -->
<article
  bind:this={row}
  tabindex="0"
  aria-label="Entry at {time}"
  class="group relative -mx-2.5 grid gap-0.5 rounded-md px-2.5 py-2 transition-[opacity,background-color,box-shadow] duration-(--dur) ease-out min-[720px]:grid-cols-[4rem_1fr] min-[720px]:gap-x-3.5 [&+&]:before:absolute [&+&]:before:inset-x-2.5 [&+&]:before:top-0 [&+&]:before:h-px [&+&]:before:bg-line
    {open ? 'bg-surface shadow-pop' : 'cursor-text hover:bg-hover'} {dimmed ? 'opacity-35' : ''}
    {editing.arrived === entry.id ? 'slide-in' : ''}"
  onclick={onrowclick}
  onkeydown={onrowkeydown}
  {oncontextmenu}
  onpointerdown={(e) => touchLike(e) && !open && press.start(e.clientX, e.clientY)}
  onpointermove={(e) => touchLike(e) && press.move(e.clientX, e.clientY)}
  onpointerup={() => press.cancel()}
  onpointercancel={() => press.cancel()}
  onanimationend={() => {
    if (editing.arrived === entry.id) editing.arrived = null;
  }}
>
  <time class="text-xs text-faint tabular-nums min-[720px]:pt-[3px] min-[720px]:text-[0.78rem]" title={full}>
    {#if writtenDate}
      <span class="mr-1.5 min-[720px]:mr-0 min-[720px]:block min-[720px]:text-[0.65rem] min-[720px]:leading-[1.3] min-[720px]:whitespace-nowrap">
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
      class="absolute top-1 right-1.5 bg-surface-2 transition-opacity min-[720px]:opacity-0 min-[720px]:group-focus-within:opacity-100 min-[720px]:group-hover:opacity-100
        {menuOpen ? 'min-[720px]:opacity-100' : ''}"
      aria-haspopup="menu"
      aria-expanded={menuOpen}
      onclick={(e) => openMenu(e.currentTarget)}
    />
  {/if}
</article>
<Menu bind:open={menuOpen} anchor={menuAnchor} items={actions} label="Entry actions" returnFocus={row} />
