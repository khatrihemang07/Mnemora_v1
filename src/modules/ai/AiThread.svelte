<!-- The Conversation as a document: each question a serif heading, each answer serif Markdown, with
     the Provider and Model that answered under it. A failed Run sits where its answer would be. -->
<script lang="ts">
  import type { EntryView } from '../../lib/ipc/bindings';
  import MarkdownView from '../../lib/ui/MarkdownView.svelte';
  import type { BannerAction } from './failure-banner';
  import FailureBanner from './FailureBanner.svelte';
  import type { LiveRun } from './run-state';

  interface Props {
    entries: EntryView[];
    /** Runs still answering in this Conversation: their text so far. */
    answering: LiveRun[];
    /** Runs that failed: their banner goes where the answer would be. */
    failed: LiveRun[];
    /** The banner's actions for one failed Run. */
    handlersFor: (run: LiveRun) => Partial<Record<BannerAction, () => void>>;
  }

  let { entries, answering, failed, handlersFor }: Props = $props();
</script>

<div class="mx-auto flex w-full max-w-[42.5rem] flex-col gap-8">
  {#each entries as entry (entry.id)}
    {#if entry.payload.kind === 'user'}
      <h2 class="font-display text-xl whitespace-pre-wrap text-ink">{entry.payload.text}</h2>
    {:else if entry.payload.kind === 'assistant'}
      <div class="flex flex-col gap-2">
        <MarkdownView source={entry.payload.text} />
        <p class="text-xs text-faint">{entry.payload.model} · {entry.payload.provider}</p>
      </div>
    {:else}
      <p class="text-xs text-faint">Using {entry.payload.model} · {entry.payload.provider_name}</p>
    {/if}
  {/each}
  {#each answering as run (run.runId)}
    {#if run.text === ''}
      <p class="text-sm text-muted">Answering…</p>
    {:else}
      <MarkdownView source={run.text} />
    {/if}
  {/each}
  {#each failed as run (run.runId)}
    {#if run.failure}
      <FailureBanner failure={run.failure} handlers={handlersFor(run)} />
    {/if}
  {/each}
</div>
