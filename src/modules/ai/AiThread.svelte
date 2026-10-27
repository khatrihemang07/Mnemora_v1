<!-- The Conversation as a document: each question a serif heading, each answer serif Markdown. -->
<script lang="ts">
  import type { EntryView } from '../../lib/ipc/bindings';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import MarkdownView from '../../lib/ui/MarkdownView.svelte';
  import type { LiveRun } from './run-state';

  interface Props {
    entries: EntryView[];
    /** Runs still answering in this Conversation: their text so far. */
    answering: LiveRun[];
    /** Runs that failed: their sentence goes where the answer would be. */
    failed: LiveRun[];
  }

  let { entries, answering, failed }: Props = $props();
</script>

<div class="mx-auto flex w-full max-w-[42.5rem] flex-col gap-8">
  {#each entries as entry (entry.id)}
    {#if entry.payload.kind === 'user'}
      <h2 class="font-display text-xl whitespace-pre-wrap text-ink">{entry.payload.text}</h2>
    {:else}
      <MarkdownView source={entry.payload.text} />
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
    <ErrorBanner error={run.failure} message={run.failure ?? 'Something went wrong.'} />
  {/each}
</div>
