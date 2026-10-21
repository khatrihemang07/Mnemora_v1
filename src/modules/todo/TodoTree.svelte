<!--
  TodoTree: Todo's navigation nested under "Todo" in the sidebar: Inbox, Today, Upcoming, All, Done,
  then a "Lists" header with a + and the Lists. The + turns into an inline name field (Enter
  creates the List and opens it). Right-click (long-press on a phone) a List for Rename, Emoji… and
  Delete list…; a List with Tasks asks what becomes of them first. Counts are open Tasks.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { tick } from 'svelte';
  import Pencil from '~icons/lucide/pencil';
  import Plus from '~icons/lucide/plus';
  import Smile from '~icons/lucide/smile';
  import Trash2 from '~icons/lucide/trash-2';
  import type { ListEntry } from '../../lib/ipc/bindings';
  import type { NavTreeProps } from '../../lib/router';
  import EmojiPicker from '../../lib/ui/EmojiPicker.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import Menu, { type MenuAnchor, type MenuItem } from '../../lib/ui/Menu.svelte';
  import DeleteListDialog from './DeleteListDialog.svelte';
  import ListGlyph from './ListGlyph.svelte';
  import { createListAndOpen, deleteListWithUndo } from './list-actions';
  import ListNameField from './ListNameField.svelte';
  import { todoNav } from './nav.svelte';
  import { invalidateTodo, listsQuery, renameList, setListEmoji } from './queries';
  import TreeRow from './TreeRow.svelte';
  import { VIEW_ICONS } from './view-icons';
  import { listView, SMART_VIEWS, sameView, smartView, type TodoView, viewCount } from './views';
  import { notifyError } from '../../lib/ui/toast';

  let { ctx }: NavTreeProps = $props();

  const sidebar = createQuery(() => listsQuery());
  const lists = $derived(sidebar.data?.lists ?? []);
  const inside = $derived(ctx.isActive('/todo'));

  const isOpen = (view: TodoView) => inside && sameView(todoNav.view, view);

  function go(view: TodoView) {
    todoNav.go(view);
    ctx.navigate('/todo');
  }

  // Creating: the + turns into a field under the header.
  let creating = $state(false);
  let plus = $state<HTMLButtonElement | null>(null);

  // Renaming one List in place.
  let renaming = $state<string | null>(null);

  // The List menu, the emoji picker and the delete Dialog all belong to one List at a time.
  let menu = $state<{ entry: ListEntry; anchor: MenuAnchor } | null>(null);
  let menuOpen = $state(false);
  let emojiFor = $state<ListEntry | null>(null);
  let emojiOpen = $state(false);
  let deleting = $state<ListEntry | null>(null);
  const rows = $state<Record<string, HTMLButtonElement | null>>({});

  const items = $derived<MenuItem[]>(
    menu
      ? [
          { label: 'Rename', icon: Pencil, onselect: () => (renaming = menu?.entry.list.id ?? null) },
          {
            label: 'Emoji…',
            icon: Smile,
            onselect: () => {
              emojiFor = menu?.entry ?? null;
              // The menu hands focus back as it closes; open the picker after that.
              void tick().then(() => (emojiOpen = true));
            },
          },
          {
            label: 'Delete list…',
            icon: Trash2,
            danger: true,
            separatorBefore: true,
            onselect: () => requestDelete(menu?.entry),
          },
        ]
      : [],
  );

  function openMenu(entry: ListEntry, x: number, y: number) {
    menu = { entry, anchor: { x, y } };
    menuOpen = true;
  }

  /** An empty List goes at once with Undo; one with Tasks asks first. */
  function requestDelete(entry: ListEntry | undefined) {
    if (!entry) return;
    if (entry.task_count === 0) void deleteListWithUndo(entry, { kind: 'delete_tasks' });
    else deleting = entry;
  }

  async function createList(name: string) {
    await createListAndOpen(name);
    creating = false;
    ctx.navigate('/todo');
  }

  async function rename(id: string, name: string) {
    await renameList(id, name);
    await invalidateTodo();
    renaming = null;
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
</script>

<div class="flex flex-col gap-0.5" role="group" aria-label="Todo">
  {#each SMART_VIEWS as v (v.id)}
    {@const Icon = VIEW_ICONS[v.id]}
    <TreeRow
      label={v.label}
      active={isOpen(smartView(v.id))}
      count={viewCount(sidebar.data, v.id).count}
      alert={viewCount(sidebar.data, v.id).alert}
      onclick={() => go(smartView(v.id))}
    >
      {#snippet glyph()}
        <span class="grid size-[1.15rem] shrink-0 place-items-center text-muted"><Icon class="size-4" /></span>
      {/snippet}
    </TreeRow>
  {/each}

  <div class="flex items-center justify-between pr-1 pl-7.5 pt-2.5 pb-0.5">
    <span class="text-xs font-medium text-faint">Lists</span>
    <IconButton
      bind:ref={plus}
      icon={Plus}
      label="New list"
      size="sm"
      onclick={() => (creating = true)}
    />
  </div>

  {#each lists as entry (entry.list.id)}
    {#if renaming === entry.list.id}
      <div class="flex min-h-8 items-center gap-2.5 rounded-sm bg-surface pr-2.5 pl-7.5">
        <ListGlyph emoji={entry.list.emoji} />
        <ListNameField
          class="flex-1"
          label="List name"
          value={entry.list.name}
          onsubmit={(name) => rename(entry.list.id, name)}
          oncancel={() => (renaming = null)}
        />
      </div>
    {:else}
      <TreeRow
        bind:ref={rows[entry.list.id]}
        label={entry.list.name}
        active={isOpen(listView(entry.list.id))}
        count={entry.open_count}
        onclick={() => go(listView(entry.list.id))}
        onmenu={(x, y) => openMenu(entry, x, y)}
      >
        {#snippet glyph()}<ListGlyph emoji={entry.list.emoji} />{/snippet}
      </TreeRow>
    {/if}
  {/each}

  {#if creating}
    <div class="flex min-h-8 items-start gap-2.5 rounded-sm bg-surface py-1.5 pr-2.5 pl-7.5">
      <ListGlyph emoji={null} class="mt-px" />
      <ListNameField
        class="flex-1"
        label="New list name"
        placeholder="List name"
        onsubmit={createList}
        oncancel={() => {
          creating = false;
          void tick().then(() => plus?.focus());
        }}
      />
    </div>
  {/if}
</div>

<Menu bind:open={menuOpen} anchor={menu?.anchor ?? null} {items} label="List actions" />

<EmojiPicker
  bind:open={emojiOpen}
  anchor={emojiFor ? (rows[emojiFor.list.id] ?? null) : null}
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
