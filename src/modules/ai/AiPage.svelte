<!--
  The AI page: a Conversation as a document, and a question box. Never focuses the box on open.
  The desktop title sits in the top bar with History, New Conversation and the Conversation menu.
  The phone header is minimal: a menu icon for the history, a small centred title, and More.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { onMount, tick } from 'svelte';
  import Copy from '~icons/lucide/copy';
  import Ellipsis from '~icons/lucide/ellipsis';
  import GitBranch from '~icons/lucide/git-branch';
  import History from '~icons/lucide/history';
  import Pencil from '~icons/lucide/pencil';
  import Plug from '~icons/lucide/plug';
  import Sparkles from '~icons/lucide/sparkles';
  import SquarePen from '~icons/lucide/square-pen';
  import Trash2 from '~icons/lucide/trash-2';
  import { commands, events, type QueuedNote } from '../../lib/ipc/bindings';
  import { queryClient } from '../../lib/ipc/query-client';
  import { IpcError, unwrap } from '../../lib/ipc/unwrap';
  import { urlParam } from '../../lib/router/url-param';
  import { readCore } from '../../lib/settings/core';
  import { appInfoQuery, settingsQuery } from '../../lib/settings/queries';
  import { viewport } from '../../lib/viewport.svelte';
  import Button from '../../lib/ui/Button.svelte';
  import Dialog from '../../lib/ui/Dialog.svelte';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import Input from '../../lib/ui/Input.svelte';
  import type { MenuItem } from '../../lib/ui/Menu.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import Popover from '../../lib/ui/Popover.svelte';
  import Sheet from '../../lib/ui/Sheet.svelte';
  import { notifyError, notifyUndo } from '../../lib/ui/toast';
  import { moduleContext } from '../../shell/module-context';
  import ActionMenu from './ActionMenu.svelte';
  import AiComposer from './AiComposer.svelte';
  import AiThread from './AiThread.svelte';
  import { type Choice, effectiveChoice } from './choice';
  import { showBranch } from './branch-actions';
  import BranchPicker from './BranchPicker.svelte';
  import ConversationHistory from './ConversationHistory.svelte';
  import { phoneBottom } from '../../lib/ui/sheet-open.svelte';
  import { announcementFor } from './announce';
  import { copyAsMarkdown } from './copy';
  import type { BannerAction } from './failure-banner';
  import { returnFocus } from './focus';
  import { aiRequests } from './ai-requests.svelte';
  import { liveRuns } from './live.svelte';
  import { localContext } from './local-context';
  import ProviderSheet from './ProviderSheet.svelte';
  import QueuedNotes from './QueuedNotes.svelte';
  import { aiKeys, conversationOptions, providersOptions, savedOptions, usageOptions } from './queries';
  import { recalledText } from './queue-state';
  import { answeringIn, bannerRuns, failedIn, lastQuestionId, type LiveRun } from './run-state';

  /** The open Conversation, in the URL so a reload and Back keep it. None is a new one. */
  const conversationId = $derived(urlParam.get('conversation'));
  const conversation = createQuery(() => ({
    ...conversationOptions(conversationId ?? ''),
    enabled: conversationId !== null,
  }));
  const providers = createQuery(() => providersOptions);
  const usage = createQuery(() => usageOptions);
  const saved = createQuery(() => savedOptions);
  const settings = createQuery(() => settingsQuery);
  const weekStart = $derived(readCore(settings.data?.values).weekStart);
  const info = createQuery(() => appInfoQuery);
  const greeting = $derived(
    info.data?.os_first_name
      ? `What should we look at, ${info.data.os_first_name}?`
      : 'What should we look at?',
  );
  const title = $derived(conversation.data?.title ?? 'New Conversation');

  /** The Model picked on this page since it opened. */
  let picked = $state<Choice | null>(null);
  let pickerOpen = $state(false);
  let addingProvider = $state(false);

  /** The history (a Popover on a desktop, a Sheet on a phone), the Conversation menu and rename. */
  let historyOpen = $state(false);
  let historyButton = $state<HTMLButtonElement | null>(null);
  let moreButton = $state<HTMLButtonElement | null>(null);
  let menuOpen = $state(false);
  let renameOpen = $state(false);
  let renameText = $state('');
  let renameProblem = $state<string | null>(null);
  let branchOpen = $state(false);
  /** The question being edited: sending the box makes an edited sibling of it. Only in this Conversation. */
  let editing = $state<{ id: string; text: string; conversationId: string | null } | null>(null);
  /** What the page's polite live region says; set once when a Run ends (never per streamed token). */
  let announcement = $state('');
  /** The button that opened Branch from earlier, so focus can go back to it (null: opened from a banner). */
  let branchOpener = $state<HTMLElement | null>(null);
  /** The button that opened the add-Provider sheet from the empty state. */
  let addOpener = $state<HTMLElement | null>(null);

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
  const busy = $derived((conversation.data?.running ?? false) || answering.length > 0);
  /** Edit ends when a Run starts or the Conversation changes. */
  const editingNow = $derived(
    busy || editing?.conversationId !== conversationId ? null : editing,
  );
  const entries = $derived(conversation.data?.entries ?? []);
  /** A failed Run keeps its banner only while its question is the last on the Branch shown. */
  const failed = $derived(
    conversationId ? bannerRuns(failedIn(liveRuns.runs, conversationId), lastQuestionId(entries)) : [],
  );
  /**
   * Notes waiting for this Conversation's Run, or left over when it ended. The live list wins once
   * this page has heard from the Run; before that (a page opened during a Run) the Conversation
   * says which notes are waiting.
   */
  const queued = $derived(
    conversationId ? (liveRuns.queued[conversationId] ?? conversation.data?.queued ?? []) : [],
  );
  const blank = $derived(entries.length === 0 && answering.length === 0 && failed.length === 0);
  /** No Provider yet and nothing said: the page explains and offers one button. */
  const firstRun = $derived(providers.isSuccess && (providers.data?.length ?? 0) === 0);

  /** The question box, for focusing it on request. */
  let composerRef = $state<HTMLTextAreaElement | null>(null);
  /** The phone tab bar's height, under the dock (the Toaster's own count for it). */
  const PHONE_TAB_BAR = 72;
  /** The bottom dock (queued notes and the composer): on a phone, toasts sit above it. */
  let dock = $state<HTMLElement | null>(null);
  $effect(() => {
    if (!viewport.isPhone || !dock) return;
    const element = dock;
    const update = () => (phoneBottom.px = element.offsetHeight + PHONE_TAB_BAR);
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => {
      observer.disconnect();
      phoneBottom.px = 0;
    };
  });

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

  /** Keys and palette actions asking the page to do something (see ai-requests). */
  $effect(() => {
    const kind = aiRequests.pending;
    if (kind === null) return;
    aiRequests.take();
    if (kind === 'composer') composerRef?.focus();
    else if (kind === 'history') historyOpen = true;
    else newConversation();
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
          void queryClient.invalidateQueries({ queryKey: aiKeys.history });
        }
        if (payload.event.type === 'entrySaved' || payload.event.type === 'runEnded') {
          void queryClient.invalidateQueries({
            queryKey: aiKeys.conversation(payload.conversation_id),
          });
        }
        if (payload.event.type === 'runEnded') {
          void queryClient.invalidateQueries({ queryKey: aiKeys.history });
          if (payload.conversation_id === conversationId) void announce(announcementFor(payload.event.outcome));
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

  /** Says `text` once: the region is cleared first so the same sentence twice is still read out. */
  async function announce(text: string) {
    announcement = '';
    await tick();
    announcement = text;
  }

  /**
   * Starts a Run with the Model in use (or the one given, for "Try again"). Resolves false when it
   * was refused, and says why.
   */
  async function send(
    text: string,
    target: { conversationId: string | null; choice: Choice } | null = null,
    branchOf: string | null = null,
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
          branch_of: branchOf,
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

  /** The text of a question in this Conversation, or null when the entry is not one. */
  function questionTextOf(questionId: string): string | null {
    const found = entries.find((entry) => entry.id === questionId);
    return found?.payload.kind === 'user' ? found.payload.text : null;
  }

  /**
   * Try again: asks a question again as its sibling, with the Provider and Model given. Resolves true
   * when the Run started.
   */
  async function askAgain(questionId: string, asked: Choice, text: string): Promise<boolean> {
    const id = conversationId;
    if (!id) return false;
    try {
      const started = await unwrap(
        commands.aiRunRetry({
          conversation_id: id,
          entry_id: questionId,
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
      return true;
    } catch (error) {
      notifyError(error instanceof Error ? error.message : 'Could not try again.');
      return false;
    }
  }

  /** Try again on a failed Run: its question is asked again with the Provider and Model it was sent with. */
  async function tryAgain(run: LiveRun) {
    const sent = liveRuns.sent[run.runId];
    if (!sent || !run.questionId) {
      notifyError('Ask the question again to retry it.');
      return;
    }
    if (await askAgain(run.questionId, sent, sent.text)) liveRuns.dismiss(run.runId);
  }

  /** Try again on an answer: its question is asked again with the Model in use on this page. */
  function tryAnswerAgain(questionId: string) {
    if (!choice) {
      notifyError('Choose a Model first.');
      return;
    }
    const text = questionTextOf(questionId);
    if (text === null) {
      notifyError('Ask the question again to retry it.');
      return;
    }
    void askAgain(questionId, choice, text);
  }

  /** Edit: the question goes into the box. Sending it makes an edited sibling; Cancel leaves Edit. */
  function startEdit(question: { id: string; text: string }) {
    editing = { ...question, conversationId };
    composerRef?.focus();
  }

  /** Sends an edited question as a sibling of the one it changes. Edit ends once it is sent. */
  async function sendEdit(text: string, questionId: string): Promise<boolean> {
    const sent = await send(text, null, questionId);
    if (sent) editing = null;
    return sent;
  }

  /** The pager: shows another question's Branch. */
  async function showOtherBranch(questionId: string, position: number) {
    const id = conversationId;
    if (!id) return;
    try {
      await showBranch(id, questionId, position);
    } catch (error) {
      notifyError(error instanceof Error ? error.message : 'Could not show that branch.');
    }
  }

  /** While a Run answers, typed text waits as a note for its next Step. Resolves true when it was queued. */
  async function addNote(text: string): Promise<boolean> {
    const id = conversationId;
    if (!id) return false;
    try {
      await unwrap(commands.aiRunSteer(id, text));
      return true;
    } catch (error) {
      notifyError(error instanceof Error ? error.message : 'Could not add the note.');
      return false;
    }
  }

  /** Stops the Run answering here. What it wrote so far is kept and marked stopped. */
  async function stopRun() {
    const id = conversationId;
    if (!id) return;
    try {
      await commands.aiRunAbort(id);
    } catch (error) {
      notifyError(error instanceof Error ? error.message : 'Could not stop the answer.');
    }
  }

  /** Sends a waiting note now: into the Run (its Step ends), or as a question when none is answering. */
  async function sendNow(note: QueuedNote) {
    const id = conversationId;
    if (!id) return;
    if (!busy) {
      if (await send(note.text)) liveRuns.dropNote(id, note.id);
      return;
    }
    try {
      await unwrap(commands.aiRunSendNow(id, note.id));
    } catch (error) {
      notifyError(error instanceof Error ? error.message : 'Could not send the note.');
    }
  }

  /**
   * Takes a waiting note back. Once a Run has ended there is nothing to tell, so it only leaves the
   * list. Resolves true when the note is no longer going to be sent.
   */
  async function takeBack(note: QueuedNote): Promise<boolean> {
    const id = conversationId;
    if (!id) return false;
    liveRuns.dropNote(id, note.id);
    if (!busy) return true;
    try {
      await unwrap(commands.aiRunUnqueue(id, note.id));
      return true;
    } catch {
      // Already sent: the Run has it, so there is nothing to take back.
      return false;
    }
  }

  /** Escape in the box during a Run: the notes not yet sent come back into the box as text. */
  async function recallNotes(): Promise<string> {
    const taken: QueuedNote[] = [];
    for (const note of queued) {
      if (await takeBack(note)) taken.push(note);
    }
    return recalledText(taken);
  }

  function handlersFor(run: LiveRun): Partial<Record<BannerAction, () => void>> {
    return {
      retry: () => void tryAgain(run),
      change_model: () => (pickerOpen = true),
      change_provider: () => (pickerOpen = true),
      branch_from_earlier: () => openBranch(null),
      new_conversation: newConversation,
    };
  }

  function newConversation() {
    urlParam.delete('conversation');
  }

  function openAddProvider(event: MouseEvent) {
    addOpener = event.currentTarget as HTMLElement;
    addingProvider = true;
  }

  function openBranch(opener: HTMLElement | null) {
    branchOpener = opener;
    branchOpen = true;
  }

  function openConversation(id: string) {
    historyOpen = false;
    urlParam.set('conversation', id);
  }

  function openRename() {
    renameText = conversation.data?.title ?? '';
    renameProblem = null;
    renameOpen = true;
  }

  async function rename() {
    const id = conversationId;
    if (!id) return;
    try {
      await unwrap(commands.aiConversationRename(id, renameText));
      renameOpen = false;
      void queryClient.invalidateQueries({ queryKey: aiKeys.conversation(id) });
      void queryClient.invalidateQueries({ queryKey: aiKeys.history });
    } catch (error) {
      if (error instanceof IpcError && error.code === 'validation') renameProblem = error.message;
      else notifyError(error instanceof Error ? error.message : 'Could not rename the Conversation.');
    }
  }

  /** Deletes at once and offers Undo, as the rest of Mnemora does. The page leaves the Conversation. */
  async function removeConversation() {
    const id = conversationId;
    if (!id) return;
    try {
      await unwrap(commands.aiConversationDelete(id));
    } catch (error) {
      notifyError(error instanceof Error ? error.message : 'Could not delete the Conversation.');
      return;
    }
    urlParam.delete('conversation');
    void queryClient.invalidateQueries({ queryKey: aiKeys.history });
    notifyUndo('Conversation deleted', () => {
      void unwrap(commands.aiConversationRestore(id)).then(() =>
        queryClient.invalidateQueries({ queryKey: aiKeys.history }),
      );
    });
  }

  /** The Conversation menu: Rename, Branch from earlier, Copy as Markdown, Delete. */
  const conversationItems = $derived<MenuItem[]>(
    conversationId
      ? [
          { label: 'Rename', icon: Pencil, onselect: openRename },
          {
            label: 'Branch from earlier...',
            icon: GitBranch,
            disabled: busy,
            onselect: () => openBranch(moreButton),
          },
          {
            label: 'Copy as Markdown',
            icon: Copy,
            onselect: () => void copyAsMarkdown(conversationId, null),
          },
          {
            label: 'Delete',
            icon: Trash2,
            danger: true,
            separatorBefore: true,
            disabled: busy,
            onselect: () => void removeConversation(),
          },
        ]
      : [],
  );
</script>

<div class="flex h-full min-h-0 flex-col">
  <p class="sr-only" role="status" aria-live="polite">{announcement}</p>
  {#if viewport.isPhone}
    <!-- The minimal phone header: the title is small, centred and always clear of the two icons. -->
    <div class="grid shrink-0 grid-cols-[2.25rem_minmax(0,1fr)_2.25rem] items-center gap-2 px-(--pad) pt-(--pad) pb-2">
      <IconButton icon={History} label="Conversations" variant="ghost" bind:ref={historyButton} onclick={() => (historyOpen = true)} />
      <h1 class="min-w-0 truncate text-center text-sm font-medium text-ink">{title}</h1>
      {#if conversationId}
        <IconButton icon={Ellipsis} label="Conversation menu" variant="ghost" bind:ref={moreButton} onclick={() => (menuOpen = true)} />
      {:else}
        <span aria-hidden="true"></span>
      {/if}
    </div>
  {:else}
    <!-- Desktop: the title goes to the top bar; PageHeader renders nothing here. -->
    <div class="shrink-0 px-(--pad) pt-(--pad) empty:hidden">
      <PageHeader title={title} icon={Sparkles} hue="--hue-ai">
        {#snippet actions()}
          <IconButton icon={History} label="Conversations" bind:ref={historyButton} onclick={() => (historyOpen = !historyOpen)} />
          <IconButton icon={SquarePen} label="New Conversation" shortcut="C" onclick={newConversation} />
          {#if conversationId}
            <IconButton icon={Ellipsis} label="Conversation menu" bind:ref={moreButton} onclick={() => (menuOpen = true)} />
          {/if}
        {/snippet}
      </PageHeader>
    </div>
  {/if}

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
          <Button variant="primary" onclick={openAddProvider}>Add a Provider</Button>
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
      {#key conversationId}
        <AiThread
          {entries}
          {answering}
          {failed}
          {handlersFor}
          conversationId={conversationId ?? ''}
          {busy}
          onedit={startEdit}
          onretry={tryAnswerAgain}
          onshow={(questionId, position) => void showOtherBranch(questionId, position)}
        />
      {/key}
    {/if}
  </div>
  <div bind:this={dock} class="px-4 pb-4">
    <QueuedNotes notes={queued} onsendnow={(note) => void sendNow(note)} onremove={(note) => void takeBack(note)} />
    <AiComposer
      {busy}
      placeholder={busy
        ? 'Add a note for the next step…'
        : entries.length === 0
          ? 'Ask about your journal and tasks…'
          : 'Ask a follow-up…'}
      choice={choice}
      {modelName}
      saved={saved.data ?? []}
      bind:ref={composerRef}
      bind:pickerOpen
      onpick={(next) => {
        picked = next;
        pickerOpen = false;
      }}
      onsend={(text) =>
        busy ? addNote(text) : editingNow ? sendEdit(text, editingNow.id) : send(text)}
      editing={editingNow}
      oncancelEdit={() => (editing = null)}
      onstop={() => void stopRun()}
      onrecall={recallNotes}
      onrunsaved={(text) => void send(text)}
      onmanage={() => moduleContext.navigate('/settings')}
    />
  </div>
</div>

{#if viewport.isPhone}
  <Sheet
    open={historyOpen}
    title="Conversations"
    onclose={() => (historyOpen = false)}
    restoreFocus={() => returnFocus(historyButton, composerRef)}
  >
    <div class="flex h-[70vh] flex-col">
      <ConversationHistory currentId={conversationId} onpick={openConversation} onnew={() => { historyOpen = false; newConversation(); }} />
    </div>
  </Sheet>
{:else}
  <Popover
    open={historyOpen}
    anchor={historyButton}
    label="Conversations"
    align="end"
    side="bottom"
    capHeight
    restoreFocus={() => returnFocus(historyButton, composerRef)}
  >
    <div class="flex min-h-0 w-[22rem] flex-col p-2">
      <ConversationHistory currentId={conversationId} onpick={openConversation} />
    </div>
  </Popover>
{/if}

<ActionMenu bind:open={menuOpen} anchor={moreButton} items={conversationItems} label="Conversation" />

<Dialog
  open={renameOpen}
  title="Rename Conversation"
  onclose={() => (renameOpen = false)}
  restoreFocus={() => returnFocus(moreButton, composerRef)}
>
  <form
    class="flex flex-col gap-2"
    onsubmit={(event) => {
      event.preventDefault();
      void rename();
    }}
  >
    <Input bind:value={renameText} label="Title" invalid={renameProblem !== null} />
    {#if renameProblem}<p role="alert" class="text-sm text-danger">{renameProblem}</p>{/if}
  </form>
  {#snippet actions()}
    <Button variant="secondary" onclick={() => (renameOpen = false)}>Cancel</Button>
    <Button variant="primary" onclick={() => void rename()}>Save</Button>
  {/snippet}
</Dialog>

<BranchPicker
  open={branchOpen}
  {conversationId}
  {entries}
  onclose={() => (branchOpen = false)}
  restoreFocus={() => returnFocus(branchOpener, composerRef)}
/>

<ProviderSheet
  open={addingProvider}
  provider={null}
  onclose={() => (addingProvider = false)}
  restoreFocus={() => returnFocus(addOpener, composerRef)}
/>
