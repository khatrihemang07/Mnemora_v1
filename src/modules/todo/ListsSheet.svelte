<!--
  ListsSheet: the phone's ☰ sheet. All and Done, then every List with its emoji and open count,
  then "New list", which turns into a name field (Enter creates the List and opens it). The row of
  the view on show is ticked. Picking a row opens that view and closes the sheet. Long-pressing a
  List opens its menu (Rename in place, Emoji… in a bottom sheet, Delete list…), the phone's
  right-click; the Inbox, All and Done have none.
-->
<script lang="ts">
  import { flushSync, tick } from 'svelte';
  import { createQuery } from '@tanstack/svelte-query';
  import Check from '~icons/lucide/check';
  import ListPlus from '~icons/lucide/list-plus';
  import Pencil from '~icons/lucide/pencil';
  import Smile from '~icons/lucide/smile';
  import Trash2 from '~icons/lucide/trash-2';
  import type { ListEntry } from '../../lib/ipc/bindings';
  import { lightTick } from '../../lib/haptic';
  import { LongPress } from '../../lib/long-press';
  import EmojiPicker from '../../lib/ui/EmojiPicker.svelte';
  import Menu, { type MenuAnchor, type MenuItem } from '../../lib/ui/Menu.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import DeleteListDialog from './DeleteListDialog.svelte';
  import Sheet from '../../lib/ui/Sheet.svelte';
  import ListGlyph from './ListGlyph.svelte';
  import { createListAndOpen, deleteListWithUndo } from './list-actions';
  import ListNameField from './ListNameField.svelte';
  import { todoNav } from './nav.svelte';
  import { invalidateTodo, listsQuery, renameList, setListEmoji } from './queries';
  import { VIEW_ICONS } from './view-icons';
  import { listView, sameView, smartView, type TodoView } from './views';

  let { open = $bindable() }: { open: boolean } = $props();

  const sidebar = createQuery(() => ({ ...listsQuery(), enabled: open }));
  const lists = $derived(sidebar.data?.lists ?? []);

  let creating = $state(false);
  $effect(() => {
    if (!open) creating = false;
  });

  function go(view: TodoView) {
    todoNav.go(view);
    open = false;
  }

  async function create(name: string) {
    await createListAndOpen(name);
    open = false;
  }

  // The List menu: one List at a time.
  let menu = $state<{ entry: ListEntry; anchor: MenuAnchor } | null>(null);
  let menuOpen = $state(false);
  let renaming = $state<string | null>(null);
  let emojiFor = $state<ListEntry | null>(null);
  let emojiOpen = $state(false);
  let deleting = $state<ListEntry | null>(null);

  function pressList(entry: ListEntry) {
    return new LongPress({
      onpress: (x, y) => {
        lightTick();
        menu = { entry, anchor: { x, y } };
        menuOpen = true;
      },
    });
  }
  const presses = new Map<string, LongPress>();
  const pressOf = (entry: ListEntry) => {
    let p = presses.get(entry.list.id);
    if (!p) presses.set(entry.list.id, (p = pressList(entry)));
    return p;
  };

  const items = $derived<MenuItem[]>(
    menu
      ? [
          {
            label: 'Rename',
            icon: Pencil,
            onselect: () => {
              renaming = menu?.entry.list.id ?? null;
              // Inside the tap's own event, so the keyboard comes up on a touch screen.
              flushSync();
              document.querySelector<HTMLInputElement>('input[aria-label="List name"]')?.select();
            },
          },
          {
            label: 'Emoji…',
            icon: Smile,
            onselect: () => {
              emojiFor = menu?.entry ?? null;
              void Promise.resolve().then(() => (emojiOpen = true));
            },
          },
          {
            label: 'Delete list…',
            icon: Trash2,
            danger: true,
            separatorBefore: true,
            onselect: () => {
              const entry = menu?.entry;
              if (!entry) return;
              // The Dialog sits at the sheet's own level and would open behind it: close the sheet first.
              open = false;
              if (entry.task_count === 0) void deleteListWithUndo(entry, { kind: 'delete_tasks' });
              else deleting = entry;
            },
          },
        ]
      : [],
  );

  async function rename(id: string, name: string) {
    await renameList(id, name);
    await invalidateTodo();
    renaming = null;
    // Touch: no visible focus ring should land on the sheet's ✕ afterwards.
    await tick();
    (document.activeElement as HTMLElement | null)?.blur();
  }

  async function pickEmoji(emoji: string | null) {
    const entry = emojiFor;
    if (!entry) return;
    try {
      await setListEmoji(entry.list.id, emoji);
    } catch {
      notifyError("Couldn't change the emoji");
    }
    void invalidateTodo();
  }

  const row =
    'flex h-12 w-full cursor-pointer items-center gap-3 rounded-sm px-3 text-left text-ink transition-colors duration-(--dur) ease-out hover:bg-hover active:bg-hover';
</script>

<Sheet {open} title="Views and Lists" onclose={() => (open = false)} closable>
  <div class="flex flex-col" role="listbox" aria-label="Views and Lists">
    {#each ['all', 'done'] as const as id (id)}
      {@const Icon = VIEW_ICONS[id]}
      <button
        type="button"
        role="option"
        aria-selected={sameView(todoNav.view, smartView(id))}
        onclick={() => go(smartView(id))}
        class={row}
      >
        <span class="grid size-6 shrink-0 place-items-center text-muted"><Icon class="size-5" /></span>
        <span class="flex-1">{id === 'all' ? 'All' : 'Done'}</span>
        {#if sameView(todoNav.view, smartView(id))}<Check class="size-4 text-hue-todo" />{/if}
      </button>
    {/each}

    <div class="px-3 pt-3 pb-1 text-xs font-medium text-faint">Lists</div>
    {#each lists as entry (entry.list.id)}
      {#if renaming === entry.list.id}
        <div class="flex min-h-12 items-start gap-3 rounded-sm bg-surface px-3 py-2.5">
          <span class="grid size-6 shrink-0 place-items-center"><ListGlyph emoji={entry.list.emoji} /></span>
          <ListNameField
            class="flex-1"
            label="List name"
            value={entry.list.name}
            onsubmit={(name) => rename(entry.list.id, name)}
            oncancel={() => (renaming = null)}
          />
        </div>
      {:else}
      <button
        type="button"
        role="option"
        aria-selected={sameView(todoNav.view, listView(entry.list.id))}
        onclick={() => (pressOf(entry).fired ? (pressOf(entry).fired = false) : go(listView(entry.list.id)))}
        onpointerdown={(e) => e.pointerType !== 'mouse' && pressOf(entry).start(e.clientX, e.clientY)}
        onpointermove={(e) => pressOf(entry).move(e.clientX, e.clientY)}
        onpointerup={() => pressOf(entry).cancel()}
        onpointercancel={() => pressOf(entry).cancel()}
        oncontextmenu={(e) => {
          e.preventDefault();
          menu = { entry, anchor: { x: e.clientX, y: e.clientY } };
          menuOpen = true;
        }}
        class="{row} select-none [-webkit-touch-callout:none]"
      >
        <span class="grid size-6 shrink-0 place-items-center"><ListGlyph emoji={entry.list.emoji} class="[&_span]:text-lg [&_svg]:size-5" /></span>
        <span class="min-w-0 flex-1 truncate">{entry.list.name}</span>
        {#if sameView(todoNav.view, listView(entry.list.id))}
          <Check class="size-4 text-hue-todo" />
        {:else if entry.open_count > 0}
          <span class="text-xs text-faint tabular-nums">{entry.open_count}</span>
        {/if}
      </button>
      {/if}
    {/each}

    {#if creating}
      <div class="flex min-h-12 items-start gap-3 rounded-sm bg-surface px-3 py-2.5">
        <span class="grid size-6 shrink-0 place-items-center"><ListGlyph emoji={null} /></span>
        <ListNameField
          class="flex-1"
          label="New list name"
          placeholder="List name"
          onsubmit={create}
          oncancel={() => (creating = false)}
        />
      </div>
    {:else}
      <button type="button" onclick={() => (creating = true)} class="{row} text-hue-todo">
        <span class="grid size-6 shrink-0 place-items-center"><ListPlus class="size-5" /></span>
        <span class="flex-1 font-medium">New list</span>
      </button>
    {/if}
  </div>
</Sheet>

<Menu bind:open={menuOpen} anchor={menu?.anchor ?? null} {items} label="List actions" />
<EmojiPicker
  bind:open={emojiOpen}
  anchor={null}
  value={emojiFor?.list.emoji ?? null}
  onpick={pickEmoji}
/>
<DeleteListDialog
  entry={deleting}
  others={lists.filter((e) => e.list.id !== deleting?.list.id).map((e) => e.list)}
  onclose={() => (deleting = null)}
  onconfirm={(choice, targetName) => {
    const entry = deleting;
    deleting = null;
    if (entry) void deleteListWithUndo(entry, choice, targetName);
  }}
/>
