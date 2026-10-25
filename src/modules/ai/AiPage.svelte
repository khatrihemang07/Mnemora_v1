<!-- The AI page: a Conversation as a document, and a question box. Never focuses the box on open. -->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { onMount } from 'svelte';
  import Plug from '~icons/lucide/plug';
  import Sparkles from '~icons/lucide/sparkles';
  import SquarePen from '~icons/lucide/square-pen';
  import { commands, events } from '../../lib/ipc/bindings';
  import { queryClient } from '../../lib/ipc/query-client';
  import { unwrap } from '../../lib/ipc/unwrap';
  import { urlParam } from '../../lib/router/url-param';
  import { readCore } from '../../lib/settings/core';
  import { appInfoQuery, settingsQuery } from '../../lib/settings/queries';
  import Button from '../../lib/ui/Button.svelte';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import AiComposer from './AiComposer.svelte';
  import AiThread from './AiThread.svelte';
  import { type Choice, effectiveChoice } from './choice';
  import type { BannerAction } from './failure-banner';
  import { liveRuns } from './live.svelte';
  import { localContext } from './local-context';
  import ProviderSheet from './ProviderSheet.svelte';
  import { aiKeys, conversationOptions, providersOptions, usageOptions } from './queries';
  import { answeringIn, failedIn, type LiveRun } from './run-state';

  /** The open Conversation, in the URL so a reload and Back keep it. None is a new one. */
  const conversationId = $derived(urlParam.get('conversation'));
  const conversation = createQuery(() => ({
    ...conversationOptions(conversationId ?? ''),
    enabled: conversationId !== null,
  }));
  const providers = createQuery(() => providersOptions);
  const usage = createQuery(() => usageOptions);
  const settings = createQuery(() => settingsQuery);
  const weekStart = $derived(readCore(settings.data?.values).weekStart);
  const info = createQuery(() => appInfoQuery);
  const greeting = $derived(
    info.data?.os_first_name
      ? `What should we look at, ${info.data.os_first_name}?`
      : 'What should we look at?',
  );

  /** The Model picked on this page since it opened. */
  let picked = $state<Choice | null>(null);
  let pickerOpen = $state(false);
  let addingProvider = $state(false);

  const choice = $derived(
    effectiveChoice({
      picked,
      conversation: conversation.data
        ? { providerId: conversation.data.provider_id, model: conversation.data.model }
        : null,
      usage: usage.data ?? null,
      providers: providers.data ?? [],
    }),
  );
  const modelName = $derived(choice?.model ?? null);

  const answering = $derived(conversationId ? answeringIn(liveRuns.runs, conversationId) : []);
  const failed = $derived(conversationId ? failedIn(liveRuns.runs, conversationId) : []);
  const busy = $derived((conversation.data?.running ?? false) || answering.length > 0);
  const entries = $derived(conversation.data?.entries ?? []);
  const blank = $derived(entries.length === 0 && answering.length === 0 && failed.length === 0);
  /** No Provider yet and nothing said: the page explains and offers one button. */
  const firstRun = $derived(providers.isSuccess && (providers.data?.length ?? 0) === 0);

  /** How close to the end, in pixels, the thread must be for new content to keep it there. */
  const END_PX = 96;
  /** The scrolling thread. It follows the newest content while it is at its end, and a new banner. */
  let scroller = $state<HTMLElement | null>(null);
  const streamed = $derived(answering.reduce((total, run) => total + run.text.length, 0));
  /** Whether the thread was at its end before the last change (measured before the DOM updates). */
  let atEnd = true;
  let failuresSeen = 0;
  $effect.pre(() => {
    void entries.length;
    void streamed;
    void failed.length;
    if (scroller) atEnd = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < END_PX;
  });
  $effect(() => {
    void entries.length;
    void streamed;
    const bannerShown = failed.length > failuresSeen;
    failuresSeen = failed.length;
    if (scroller && (atEnd || bannerShown)) scroller.scrollTo({ top: scroller.scrollHeight });
  });

  onMount(() => {
    // Runs seen before this page opened are not in the saved Conversation's live state any more.
    liveRuns.clear();
    let stopListening: (() => void) | undefined;
    let disposed = false;
    void events.aiRunEvent
      .listen(({ payload }) => {
        liveRuns.apply(payload);
        if (payload.event.type === 'runStarted') {
          void queryClient.invalidateQueries({ queryKey: aiKeys.usage });
        }
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

  /**
   * Starts a Run with the Model in use (or the one given, for "Try again"). Resolves false when it
   * was refused, and says why.
   */
  async function send(
    text: string,
    target: { conversationId: string | null; choice: Choice } | null = null,
  ): Promise<boolean> {
    const asked = target?.choice ?? choice;
    if (!asked) {
      notifyError('Choose a Model first.');
      return false;
    }
    const inConversation = target ? target.conversationId : conversationId;
    try {
      const started = await unwrap(
        commands.aiRunStart({
          conversation_id: inConversation,
          text,
          provider_id: asked.providerId,
          model: asked.model,
          local: localContext(new Date(), weekStart, Intl.DateTimeFormat().resolvedOptions().timeZone),
        }),
      );
      liveRuns.remember(started.run_id, {
        text,
        providerId: asked.providerId,
        model: asked.model,
        conversationId: started.conversation_id,
      });
      if (started.conversation_id !== conversationId) {
        urlParam.set('conversation', started.conversation_id, true);
      }
      return true;
    } catch (error) {
      notifyError(error instanceof Error ? error.message : 'Could not send the question.');
      return false;
    }
  }

  /** Asks the failed question again, with the Provider and Model it was sent with. */
  async function tryAgain(run: LiveRun) {
    const sent = liveRuns.sent[run.runId];
    if (!sent) {
      notifyError('Ask the question again to retry it.');
      return;
    }
    const started = await send(sent.text, {
      conversationId: sent.conversationId,
      choice: { providerId: sent.providerId, model: sent.model },
    });
    if (started) liveRuns.dismiss(run.runId);
  }

  function handlersFor(run: LiveRun): Partial<Record<BannerAction, () => void>> {
    return {
      retry: () => void tryAgain(run),
      change_model: () => (pickerOpen = true),
      change_provider: () => (pickerOpen = true),
      new_conversation: newConversation,
    };
  }

  function newConversation() {
    urlParam.delete('conversation');
  }
</script>

<div class="flex h-full min-h-0 flex-col">
  <!-- Phone shows a large title here; on desktop PageHeader renders nothing and the box collapses. -->
  <div class="shrink-0 px-(--pad) pt-(--pad) empty:hidden">
    <PageHeader title="AI" icon={Sparkles} hue="--hue-ai">
      {#snippet actions()}
        <IconButton icon={SquarePen} label="New Conversation" onclick={newConversation} />
      {/snippet}
    </PageHeader>
  </div>
  <div bind:this={scroller} class="min-h-0 flex-1 overflow-y-auto px-4 pt-6 pb-10">
    {#if conversation.isError && conversationId}
      <ErrorBanner error={conversation.error} onretry={() => void conversation.refetch()} />
    {:else if firstRun && blank}
      <EmptyState
        icon={Plug}
        title="Add a Provider to start"
        description="A Provider is where answers come from: the Local AI Wrapper on this machine, or another service you choose. Your key stays on this device."
        hue="--hue-ai"
      >
        {#snippet action()}
          <Button variant="primary" onclick={() => (addingProvider = true)}>Add a Provider</Button>
        {/snippet}
      </EmptyState>
    {:else if blank}
      <EmptyState
        icon={Sparkles}
        title={greeting}
        description="Answers use your Journal and Tasks."
        hue="--hue-ai"
      />
    {:else}
      <AiThread {entries} {answering} {failed} {handlersFor} />
    {/if}
  </div>
  <div class="px-4 pb-4">
    <AiComposer
      {busy}
      placeholder={entries.length === 0 ? 'Ask about your journal and tasks…' : 'Ask a follow-up…'}
      choice={choice}
      {modelName}
      bind:pickerOpen
      onpick={(next) => {
        picked = next;
        pickerOpen = false;
      }}
      onsend={(text) => send(text)}
    />
  </div>
</div>

<ProviderSheet open={addingProvider} provider={null} onclose={() => (addingProvider = false)} />
