<!--
  ListPicker: choose where a Task goes: the Inbox, then every List with its emoji. A popover next
  to `anchor` on desktop, a bottom Sheet on a phone. `current` is the List the Task is in now
  (null is the Inbox), marked with a tick. `onpick` gets the List, or null for the Inbox.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import Check from '~icons/lucide/check';
  import type { TodoList } from '../../lib/ipc/bindings';
  import Popover from '../../lib/ui/Popover.svelte';
  import Sheet from '../../lib/ui/Sheet.svelte';
  import { viewport } from '../../lib/viewport.svelte';
  import ListGlyph from './ListGlyph.svelte';
  import { listsQuery } from './queries';
  import { VIEW_ICONS } from './view-icons';

  interface Props {
    open: boolean;
    anchor: HTMLElement | null;
    current: string | null;
    onpick: (list: TodoList | null) => void;
    title?: string;
  }
  let { open = $bindable(), anchor, current, onpick, title = 'Move to list' }: Props = $props();

  const sidebar = createQuery(() => ({ ...listsQuery(), enabled: open }));
  const Inbox = VIEW_ICONS.inbox;

  function pick(list: TodoList | null) {
    open = false;
    onpick(list);
  }

  const row =
    'flex h-9 w-full cursor-pointer items-center gap-2.5 rounded-sm px-2.5 text-left text-sm text-ink transition-colors duration-(--dur) ease-out hover:bg-hover desktop:h-8';
</script>

{#snippet body()}
  <div class="flex max-h-[min(60vh,22rem)] min-w-48 flex-col overflow-y-auto" role="listbox" aria-label={title}>
    <button type="button" role="option" aria-selected={current === null} onclick={() => pick(null)} class={row}>
      <span class="grid size-[1.15rem] shrink-0 place-items-center text-muted"><Inbox class="size-4" /></span>
      <span class="flex-1">Inbox</span>
      {#if current === null}<Check class="size-4 text-accent" />{/if}
    </button>
    {#each sidebar.data?.lists ?? [] as entry (entry.list.id)}
      <button
        type="button"
        role="option"
        aria-selected={current === entry.list.id}
        onclick={() => pick(entry.list)}
        class={row}
      >
        <ListGlyph emoji={entry.list.emoji} />
        <span class="min-w-0 flex-1 truncate">{entry.list.name}</span>
        {#if current === entry.list.id}<Check class="size-4 text-accent" />{/if}
      </button>
    {/each}
  </div>
{/snippet}

{#if viewport.isPhone}
  <Sheet {open} {title} onclose={() => (open = false)} closable>{@render body()}</Sheet>
{:else}
  <Popover bind:open {anchor} label={title} flush focus="[role=option]">{@render body()}</Popover>
{/if}
