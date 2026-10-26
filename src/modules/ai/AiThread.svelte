<!-- The Conversation as a document: each question a serif heading, each answer serif Markdown, with
     the Provider and Model that answered under it. A Run in progress or failed sits under the
     question it answers, so a failed Run's banner stays in order with the rest of the thread. -->
<script lang="ts">
  import Bookmark from '~icons/lucide/bookmark';
  import Copy from '~icons/lucide/copy';
  import type { EntryView } from '../../lib/ipc/bindings';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import MarkdownView from '../../lib/ui/MarkdownView.svelte';
  import { notifyError, notify } from '../../lib/ui/toast';
  import Activity from './Activity.svelte';
  import { questionsByAnswer } from './answers';
  import { copyAsMarkdown } from './copy';
  import { stepsByQuestion, withoutCitations } from './activity';
  import type { BannerAction } from './failure-banner';
  import FailureBanner from './FailureBanner.svelte';
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
  }

  let { entries, answering, failed, handlersFor, conversationId }: Props = $props();
  /** The question each answer sits under, for "Save this question". */
  const questions = $derived(questionsByAnswer(entries));

  async function saveAsQuestion(text: string) {
    try {
      await saveQuestion(text);
      notify('Saved as a Saved question');
    } catch (failure) {
      notifyError(failure instanceof Error ? failure.message : 'Could not save the question.');
    }
  }

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

<div class="mx-auto flex w-full max-w-[42.5rem] flex-col gap-8">
  {#each entries as entry (entry.id)}
    {#if entry.payload.kind === 'user'}
      {@const liveHere = runsUnder(answering, entry.id).length + runsUnder(failed, entry.id).length}
      <div class="flex flex-col gap-4">
        <h2 class="font-display text-xl whitespace-pre-wrap text-ink">{entry.payload.text}</h2>
        {#if liveHere === 0 && (steps.get(entry.id)?.length ?? 0) > 0}
          <Activity steps={steps.get(entry.id) ?? []} running={false} />
        {/if}
        {@render runsView(runsUnder(answering, entry.id), runsUnder(failed, entry.id))}
      </div>
    {:else if entry.payload.kind === 'assistant' && entry.payload.calls.length === 0}
      <div class="flex flex-col gap-3">
        <MarkdownView source={withoutCitations(entry.payload.text)} />
        <Sources sources={entry.payload.sources} />
        {#if entry.payload.budget_note}
          <p class="text-sm text-muted">{entry.payload.budget_note}</p>
        {/if}
        <div class="flex flex-wrap items-center gap-1">
          <IconButton icon={Copy} label="Copy answer as Markdown" size="sm" variant="ghost" onclick={() => void copyAsMarkdown(conversationId, entry.id)} />
          {#if questions.get(entry.id)}
            {@const question = questions.get(entry.id) ?? ''}
            <IconButton icon={Bookmark} label="Save this question" size="sm" variant="ghost" onclick={() => void saveAsQuestion(question)} />
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
