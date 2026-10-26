<!-- The Conversation as a document: each question a serif heading, each answer serif Markdown, with
     the Provider and Model that answered under it. A Run in progress or failed sits under the
     question it answers, so a failed Run's banner stays in order with the rest of the thread.
     A question with siblings shows its pager; Edit is a pencil on the heading (on a phone, a long
     press on the question shows it). Try again sits under each answer. -->
<script lang="ts">
  import Bookmark from '~icons/lucide/bookmark';
  import ChevronLeft from '~icons/lucide/chevron-left';
  import ChevronRight from '~icons/lucide/chevron-right';
  import Copy from '~icons/lucide/copy';
  import Pencil from '~icons/lucide/pencil';
  import RotateCcw from '~icons/lucide/rotate-ccw';
  import { onDestroy } from 'svelte';
  import type { EntryView } from '../../lib/ipc/bindings';
  import { viewport } from '../../lib/viewport.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import MarkdownView from '../../lib/ui/MarkdownView.svelte';
  import { notifyError, notify } from '../../lib/ui/toast';
  import Activity from './Activity.svelte';
  import { questionOfAnswers } from './answers';
  import { pagerFocusAfterShow, pagerLabel, pagerStep } from './branch-pager';
  import { copyAsMarkdown } from './copy';
  import { stepsByQuestion, withoutCitations } from './activity';
  import type { BannerAction } from './failure-banner';
  import FailureBanner from './FailureBanner.svelte';
  import { LONG_PRESS_MS, type Point, pressMovedTooFar } from './long-press';
  import { type LiveRun, runsUnder, unplacedRuns } from './run-state';
  import { saveQuestion } from './saved-actions';
  import Sources from './Sources.svelte';

  interface Props {
    entries: EntryView[];
    /** Runs still answering in this Conversation: their text so far. */
    answering: LiveRun[];
    /** Runs that failed: their banner goes under the question that failed. */
    failed: LiveRun[];
    /** The banner's actions for one failed Run. */
    handlersFor: (run: LiveRun) => Partial<Record<BannerAction, () => void>>;
    /** The Conversation the entries belong to (copies name it). */
    conversationId: string;
    /** A Run is answering or a Branch is being changed: the Branch tools are off. */
    busy: boolean;
    /** Edit: the question's entry id and text, for the question box. */
    onedit: (question: { id: string; text: string }) => void;
    /** Try again on an answer: its question's entry id. */
    onretry: (questionId: string) => void;
    /** The pager: the question shown beside `questionId` moves to `position` (from 1). */
    onshow: (questionId: string, position: number) => void;
  }

  let { entries, answering, failed, handlersFor, conversationId, busy, onedit, onretry, onshow }: Props =
    $props();
  /** Each answer's question (id and text): for Save this question and Try again. */
  const questions = $derived(questionOfAnswers(entries));

  async function saveAsQuestion(text: string) {
    try {
      await saveQuestion(text);
      notify('Saved as a Saved question');
    } catch (failure) {
      notifyError(failure instanceof Error ? failure.message : 'Could not save the question.');
    }
  }

  /** The question whose Edit control a long press revealed on a phone, until the page changes. */
  let revealed = $state<string | null>(null);
  let hold: { id: string; start: Point; timer: ReturnType<typeof setTimeout> } | null = null;

  function cancelHold() {
    if (hold) clearTimeout(hold.timer);
    hold = null;
  }

  /** A phone's long press on a question reveals its Edit control. Desktop shows it on hover instead. */
  function startHold(id: string, event: PointerEvent) {
    if (!viewport.isPhone) return;
    cancelHold();
    const start = { x: event.clientX, y: event.clientY };
    hold = {
      id,
      start,
      timer: setTimeout(() => {
        revealed = id;
        hold = null;
      }, LONG_PRESS_MS),
    };
  }

  function moveHold(event: PointerEvent) {
    if (hold && pressMovedTooFar(hold.start, { x: event.clientX, y: event.clientY })) cancelHold();
  }

  onDestroy(cancelHold);

  /**
   * A pager press waiting for the Branch it switches to. Showing a sibling swaps the question's
   * element, so focus is put back on the equivalent arrow once the thread has re-rendered.
   */
  let pendingPager: { position: number; pressed: 'previous' | 'next' } | null = null;
  let thread = $state<HTMLElement | null>(null);

  $effect(() => {
    void entries;
    const pending = pendingPager;
    if (!pending || !thread) return;
    pendingPager = null;
    const group = thread.querySelector<HTMLElement>(`[data-question-at="${pending.position}"]`);
    const previous = group?.querySelector<HTMLButtonElement>('button[aria-label="Previous branch"]');
    const next = group?.querySelector<HTMLButtonElement>('button[aria-label="Next branch"]');
    const target = pagerFocusAfterShow(pending.pressed, {
      previous: previous?.disabled === false,
      next: next?.disabled === false,
    });
    if (target === 'previous') previous?.focus();
    else if (target === 'next') next?.focus();
  });

  const entryIds = $derived(new Set(entries.map((entry) => entry.id)));
  /** The Tool steps each saved question made, shown once its Run has no live copy. */
  const steps = $derived(stepsByQuestion(entries));
  /** Runs whose question is not in the thread yet: they go at the end, as before. */
  const loose = $derived({
    answering: unplacedRuns(answering, entryIds),
    failed: unplacedRuns(failed, entryIds),
  });
</script>

{#snippet runsView(answeringRuns: LiveRun[], failedRuns: LiveRun[])}
  {#each answeringRuns as run (run.runId)}
    <Activity steps={run.steps} running />
    {#if run.text === ''}
      <p class="text-sm text-muted">Answering…</p>
    {:else}
      <MarkdownView source={withoutCitations(run.text)} />
    {/if}
  {/each}
  {#each failedRuns as run (run.runId)}
    {#if run.failure}
      <FailureBanner failure={run.failure} handlers={handlersFor(run)} />
    {/if}
  {/each}
{/snippet}

<div bind:this={thread} class="mx-auto flex w-full max-w-[42.5rem] flex-col gap-8">
  {#each entries as entry, index (entry.id)}
    {#if entry.payload.kind === 'user'}
      {@const liveHere = runsUnder(answering, entry.id).length + runsUnder(failed, entry.id).length}
      {@const previous = pagerStep(entry.sibling_index, entry.sibling_count, 'previous')}
      {@const next = pagerStep(entry.sibling_index, entry.sibling_count, 'next')}
      {@const questionText = entry.payload.text}
      <div class="flex flex-col gap-4" data-question-at={index}>
        <div
          role="group"
          aria-label="Question"
          class="group flex flex-wrap items-center gap-x-2 gap-y-1"
          onpointerdown={(event) => startHold(entry.id, event)}
          onpointermove={moveHold}
          onpointerup={cancelHold}
          onpointercancel={cancelHold}
          onpointerleave={cancelHold}
        >
          <h2 class="min-w-0 font-display text-xl break-words whitespace-pre-wrap text-ink">{entry.payload.text}</h2>
          <div class="flex items-center gap-0.5">
            {#if entry.sibling_count > 1}
              <IconButton
                icon={ChevronLeft}
                label="Previous branch"
                size="sm"
                variant="ghost"
                disabled={busy || previous === null}
                onclick={() => {
                  if (previous === null) return;
                  pendingPager = { position: index, pressed: 'previous' };
                  onshow(entry.id, previous);
                }}
              />
              <span class="px-0.5 text-xs whitespace-nowrap text-muted">
                {pagerLabel(entry.sibling_index, entry.sibling_count)}
              </span>
              <IconButton
                icon={ChevronRight}
                label="Next branch"
                size="sm"
                variant="ghost"
                disabled={busy || next === null}
                onclick={() => {
                  if (next === null) return;
                  pendingPager = { position: index, pressed: 'next' };
                  onshow(entry.id, next);
                }}
              />
            {/if}
            {#if !viewport.isPhone || revealed === entry.id}
              <span
                class="flex transition-opacity duration-(--dur) ease-out {viewport.isPhone
                  ? ''
                  : 'opacity-0 group-focus-within:opacity-100 desktop:group-hover:opacity-100'}"
              >
                <IconButton
                  icon={Pencil}
                  label="Edit question"
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  onclick={() => onedit({ id: entry.id, text: questionText })}
                />
              </span>
            {/if}
          </div>
        </div>
        {#if liveHere === 0 && (steps.get(entry.id)?.length ?? 0) > 0}
          <Activity steps={steps.get(entry.id) ?? []} running={false} />
        {/if}
        {@render runsView(runsUnder(answering, entry.id), runsUnder(failed, entry.id))}
      </div>
    {:else if entry.payload.kind === 'assistant' && entry.payload.calls.length === 0}
      <div class="flex flex-col gap-3">
        <MarkdownView source={withoutCitations(entry.payload.text)} />
        {#if entry.payload.stop === 'aborted'}
          <p class="text-sm text-muted">Stopped</p>
        {/if}
        <Sources sources={entry.payload.sources} />
        {#if entry.payload.budget_note}
          <p class="text-sm text-muted">{entry.payload.budget_note}</p>
        {/if}
        <div class="flex flex-wrap items-center gap-1">
          <IconButton icon={Copy} label="Copy answer as Markdown" size="sm" variant="ghost" onclick={() => void copyAsMarkdown(conversationId, entry.id)} />
          {#if questions.get(entry.id)}
            {@const question = questions.get(entry.id)}
            <IconButton icon={Bookmark} label="Save this question" size="sm" variant="ghost" onclick={() => void saveAsQuestion(question?.text ?? '')} />
            <IconButton icon={RotateCcw} label="Try again" size="sm" variant="ghost" disabled={busy} onclick={() => onretry(question?.id ?? '')} />
          {/if}
          <span class="ml-1 text-xs text-muted">{entry.payload.model} · {entry.payload.provider}</span>
        </div>
      </div>
    {:else if entry.payload.kind === 'model_change'}
      <p class="text-xs text-muted">Using {entry.payload.model} · {entry.payload.provider_name}</p>
    {/if}
  {/each}
  {@render runsView(loose.answering, loose.failed)}
</div>
