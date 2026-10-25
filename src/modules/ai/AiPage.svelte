<!-- The AI page: a Conversation as a document, and a question box. Never focuses the box on open. -->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { onMount } from 'svelte';
  import Sparkles from '~icons/lucide/sparkles';
  import SquarePen from '~icons/lucide/square-pen';
  import { commands, events } from '../../lib/ipc/bindings';
  import { queryClient } from '../../lib/ipc/query-client';
  import { unwrap } from '../../lib/ipc/unwrap';
  import { urlParam } from '../../lib/router/url-param';
  import { readCore } from '../../lib/settings/core';
  import { appInfoQuery, settingsQuery } from '../../lib/settings/queries';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import AiComposer from './AiComposer.svelte';
  import AiThread from './AiThread.svelte';
  import { liveRuns } from './live.svelte';
  import { localContext } from './local-context';
  import { aiKeys, conversationOptions } from './queries';
  import { answeringIn, failedIn } from './run-state';

  /** The open Conversation, in the URL so a reload and Back keep it. None is a new one. */
  const conversationId = $derived(urlParam.get('conversation'));
  const conversation = createQuery(() => ({
    ...conversationOptions(conversationId ?? ''),
    enabled: conversationId !== null,
  }));
  const settings = createQuery(() => settingsQuery);
  const weekStart = $derived(readCore(settings.data?.values).weekStart);
  const info = createQuery(() => appInfoQuery);
  const greeting = $derived(
    info.data?.os_first_name
      ? `What should we look at, ${info.data.os_first_name}?`
      : 'What should we look at?',
  );

  const answering = $derived(conversationId ? answeringIn(liveRuns.runs, conversationId) : []);
  const failed = $derived(conversationId ? failedIn(liveRuns.runs, conversationId) : []);
  const busy = $derived((conversation.data?.running ?? false) || answering.length > 0);
  const entries = $derived(conversation.data?.entries ?? []);
  const blank = $derived(entries.length === 0 && answering.length === 0 && failed.length === 0);

  onMount(() => {
    // Runs seen before this page opened are not in the saved Conversation's live state any more.
    liveRuns.clear();
    let stopListening: (() => void) | undefined;
    let disposed = false;
    void events.aiRunEvent
      .listen(({ payload }) => {
        liveRuns.apply(payload);
        if (payload.event.type === 'entrySaved' || payload.event.type === 'runEnded') {
          void queryClient.invalidateQueries({
            queryKey: aiKeys.conversation(payload.conversation_id),
          });
        }
      })
      .then((stop) => {
        if (disposed) stop();
        else stopListening = stop;
      });
    return () => {
      disposed = true;
      stopListening?.();
    };
  });

  /** Starts a Run. Resolves false when it was refused, and says why. */
  async function send(text: string): Promise<boolean> {
    try {
      const started = await unwrap(
        commands.aiRunStart({
          conversation_id: conversationId,
          text,
          local: localContext(new Date(), weekStart, Intl.DateTimeFormat().resolvedOptions().timeZone),
        }),
      );
      if (started.conversation_id !== conversationId) {
        urlParam.set('conversation', started.conversation_id, true);
      }
      return true;
    } catch (error) {
      notifyError(error instanceof Error ? error.message : 'Could not send the question.');
      return false;
    }
  }

  function newConversation() {
    urlParam.delete('conversation');
  }
</script>

<div class="flex h-full min-h-0 flex-col">
  <PageHeader title="AI" icon={Sparkles} hue="--hue-ai">
    {#snippet actions()}
      <IconButton icon={SquarePen} label="New Conversation" onclick={newConversation} />
    {/snippet}
  </PageHeader>
  <div class="min-h-0 flex-1 overflow-y-auto px-4 pt-6 pb-10">
    {#if conversation.isError && conversationId}
      <ErrorBanner error={conversation.error} onretry={() => void conversation.refetch()} />
    {:else if blank}
      <EmptyState
        icon={Sparkles}
        title={greeting}
        description="Answers use your Journal and Tasks."
        hue="--hue-ai"
      />
    {:else}
      <AiThread {entries} {answering} {failed} />
    {/if}
  </div>
  <div class="px-4 pb-4">
    <AiComposer
      {busy}
      placeholder={entries.length === 0 ? 'Ask about your journal and tasks…' : 'Ask a follow-up…'}
      onsend={send}
    />
  </div>
</div>
