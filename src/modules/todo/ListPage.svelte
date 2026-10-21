<!--
  ListPage: one List: its emoji (click to change it), its title (click to rename it in place) and its
  open Tasks, with "+ New task" adding into this List. "Group:" (None, Priority, Due date) and
  "Sort:" (Manual, Due date, Priority, Title, Recently added) belong to this List: grouping happens
  first and the sort orders each group, and dragging is off while sorted. Done Tasks follow the
  Done-in-Lists Setting. If the List is gone (deleted elsewhere), Todo falls back to the Inbox.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import EmojiPicker from '../../lib/ui/EmojiPicker.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import { canDrag, GROUP_OPTIONS } from './arrange';
  import { todoClock } from './clock.svelte';
  import ListDoneSection from './ListDoneSection.svelte';
  import TodoActions from './TodoActions.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import ListGlyph from './ListGlyph.svelte';
  import ListNameField from './ListNameField.svelte';
  import { todoNav } from './nav.svelte';
  import {
    invalidateTodo,
    listQuery,
    renameList,
    setListEmoji,
    setListGroup,
    setListSort,
  } from './queries';
  import { useSidebar } from './sidebar.svelte';
  import TaskGroups from './TaskGroups.svelte';
  import ViewOptions from './ViewOptions.svelte';
  import { smartView } from './views';

  let { id }: { id: string } = $props();

  const page = createQuery(() => listQuery(id, todoClock.today));
  const sidebar = useSidebar();
  const list = $derived(page.data?.list ?? null);

  $effect(() => {
    if (page.isError && !page.isFetching && 'code' in page.error && page.error.code === 'not_found') {
      todoNav.go(smartView('inbox'));
    }
  });

  let renaming = $state(false);
  let emojiOpen = $state(false);
  let emojiButton = $state<HTMLButtonElement | null>(null);
  let titleButton = $state<HTMLButtonElement | null>(null);

  async function rename(name: string) {
    await renameList(id, name);
    await invalidateTodo();
    renaming = false;
  }

  /** Saves one of this List's own choices; the page comes back arranged by it. */
  async function arrange(save: Promise<unknown>) {
    try {
      await save;
    } catch {
      notifyError("Couldn't change the List");
    }
    void invalidateTodo();
  }

  async function pickEmoji(emoji: string | null) {
    try {
      await setListEmoji(id, emoji);
    } catch {
      notifyError("Couldn't change the emoji");
    }
    void invalidateTodo();
  }
</script>

{#snippet heading()}
  {#if list}
    <button
      bind:this={emojiButton}
      type="button"
      aria-label="Change emoji"
      onclick={() => (emojiOpen = !emojiOpen)}
      class="grid size-8 shrink-0 cursor-pointer place-items-center rounded-sm text-xl transition-colors duration-(--dur) ease-out hover:bg-hover"
    >
      <ListGlyph emoji={list.emoji} class="[&_span]:text-xl [&_svg]:size-5 [&_svg]:text-hue-todo" />
    </button>
    {#if renaming}
      <ListNameField
        class="min-w-40 font-display text-lg"
        label="List name"
        value={list.name}
        floating
        onsubmit={rename}
        oncancel={() => {
          renaming = false;
          queueMicrotask(() => titleButton?.focus());
        }}
      />
    {:else}
      <h1
        class="min-w-0 truncate font-display text-lg phone:text-2xl"
        style="font-weight: var(--display-weight); letter-spacing: var(--display-track)"
      >
        <button
          bind:this={titleButton}
          type="button"
          aria-label="Rename list: {list.name}"
          onclick={() => (renaming = true)}
          class="cursor-text rounded-sm px-1 -mx-1 text-left transition-colors duration-(--dur) ease-out hover:bg-hover"
        >
          {list.name}
        </button>
      </h1>
    {/if}
  {/if}
{/snippet}

<PageHeader title={list?.name ?? 'List'} {heading}>
  {#snippet actions()}<TodoActions />{/snippet}
</PageHeader>
<EmojiPicker bind:open={emojiOpen} anchor={emojiButton} value={list?.emoji ?? null} onpick={pickEmoji} />
<div class="mx-auto max-w-3xl">
  {#if page.isPending}
    <div class="flex flex-col gap-3 py-2">
      <Skeleton class="h-5 w-64" />
      <Skeleton class="h-5 w-48" />
    </div>
  {:else if page.isError}
    <ErrorBanner message="The List couldn't load." error={page.error} onretry={() => page.refetch()} />
  {:else}
    <ViewOptions
      class="mb-1 justify-end"
      groupBy={page.data.list.group_by}
      groupOptions={GROUP_OPTIONS.list}
      ongroup={(next) => void arrange(setListGroup(id, next))}
      sort={page.data.list.task_sort}
      onsort={(next) => void arrange(setListSort(id, next))}
    />
    <TaskGroups
      groups={page.data.groups}
      today={todoClock.today}
      lists={sidebar.lists}
      arrangeable={canDrag(page.data.list.task_sort)}
    />
    {#if page.data.groups.every((g) => g.tasks.length === 0)}
      <p class="px-2 pt-1 text-sm text-faint">No Tasks in this List yet.</p>
    {/if}
    <ListDoneSection done={page.data.done} />
  {/if}
</div>
