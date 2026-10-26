<!--
  The history of Conversations: a search over their titles, grouped by day and 30 at a time. It is
  the body of a Popover on a desktop and of a Sheet on a phone. It loads only while it is shown.
-->
<script lang="ts">
  import { createInfiniteQuery } from '@tanstack/svelte-query';
  import { onMount } from 'svelte';
  import MessageSquare from '~icons/lucide/message-square';
  import Plus from '~icons/lucide/plus';
  import Button from '../../lib/ui/Button.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import Input from '../../lib/ui/Input.svelte';
  import { groupConversations, shortDate, updatedAt } from './history';
  import { historyOptions } from './queries';

  interface Props {
    /** The Conversation open on the page, marked as current. */
    currentId: string | null;
    /** Opens a Conversation. */
    onpick: (id: string) => void;
    /** Starts a new one. Shown on a phone, where the header has no New button of its own. */
    onnew?: () => void;
  }

  let { currentId, onpick, onnew }: Props = $props();

  let query = $state('');
  let searchRef = $state<HTMLInputElement | null>(null);
  const history = createInfiniteQuery(() => historyOptions(query));
  const items = $derived(history.data?.pages.flatMap((page) => page.items) ?? []);
  const sections = $derived(groupConversations(items, new Date()));

  onMount(() => searchRef?.focus());
</script>

<div class="flex min-h-0 flex-col gap-3">
  <Input bind:value={query} bind:ref={searchRef} label="Search Conversations" placeholder="Search Conversations…" />

  <div class="flex min-h-0 flex-col gap-3 overflow-y-auto">
    {#if history.isError}
      <ErrorBanner error={history.error} onretry={() => void history.refetch()} />
    {:else if history.isPending}
      <p class="px-2 text-sm text-muted">Loading…</p>
    {:else if sections.length === 0}
      <p class="px-2 text-sm text-muted">
        {query.trim() === '' ? 'No Conversations yet.' : 'No Conversations match that search.'}
      </p>
    {/if}

    {#each sections as section (section.group)}
      <div class="flex flex-col gap-0.5">
        <p class="px-2 pt-1 text-xs font-medium text-muted">{section.label}</p>
        {#each section.items as conversation (conversation.id)}
          <button
            type="button"
            aria-current={conversation.id === currentId ? 'true' : undefined}
            class="flex min-h-9 items-center gap-2.5 rounded-sm px-2 py-1.5 text-left text-sm transition-colors duration-(--dur) ease-out hover:bg-hover"
            onclick={() => onpick(conversation.id)}
          >
            <MessageSquare class="size-4 shrink-0 text-muted" />
            <span class="min-w-0 flex-1 truncate text-ink">{conversation.title}</span>
            <span class="shrink-0 text-xs text-muted">{shortDate(updatedAt(conversation))}</span>
          </button>
        {/each}
      </div>
    {/each}

    {#if history.hasNextPage}
      <Button variant="ghost" size="sm" disabled={history.isFetchingNextPage} onclick={() => void history.fetchNextPage()}>
        Show more
      </Button>
    {/if}
  </div>

  {#if onnew}
    <Button variant="primary" onclick={onnew}><Plus class="size-4" /> New Conversation</Button>
  {/if}
</div>
