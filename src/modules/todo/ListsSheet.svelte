<!--
  ListsSheet: the phone's ☰ sheet. All and Done, then every List with its emoji and open count,
  then "New list", which turns into a name field (Enter creates the List and opens it). The row of
  the view on show is ticked. Picking a row opens that view and closes the sheet.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import Check from '~icons/lucide/check';
  import ListPlus from '~icons/lucide/list-plus';
  import Sheet from '../../lib/ui/Sheet.svelte';
  import ListGlyph from './ListGlyph.svelte';
  import { createListAndOpen } from './list-actions';
  import ListNameField from './ListNameField.svelte';
  import { todoNav } from './nav.svelte';
  import { listsQuery } from './queries';
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
        {#if sameView(todoNav.view, smartView(id))}<Check class="size-4 text-accent" />{/if}
      </button>
    {/each}

    <div class="px-3 pt-3 pb-1 text-xs font-medium text-faint">Lists</div>
    {#each lists as entry (entry.list.id)}
      <button
        type="button"
        role="option"
        aria-selected={sameView(todoNav.view, listView(entry.list.id))}
        onclick={() => go(listView(entry.list.id))}
        class={row}
      >
        <span class="grid size-6 shrink-0 place-items-center"><ListGlyph emoji={entry.list.emoji} class="[&_span]:text-lg [&_svg]:size-5" /></span>
        <span class="min-w-0 flex-1 truncate">{entry.list.name}</span>
        {#if sameView(todoNav.view, listView(entry.list.id))}
          <Check class="size-4 text-accent" />
        {:else if entry.open_count > 0}
          <span class="text-xs text-faint tabular-nums">{entry.open_count}</span>
        {/if}
      </button>
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
