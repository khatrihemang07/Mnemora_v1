<!--
  ListDoneSection: a List's Done Tasks, as the Done-in-Lists Setting says. Tasks done today (or all,
  with "All shown") sit struck through at the bottom; the rest wait behind a fold: "Done earlier · N"
  after "Today shown", or "Done · N" with "All folded". A done Task's diamond brings it back.
-->
<script lang="ts">
  import ChevronDown from '~icons/lucide/chevron-down';
  import ChevronRight from '~icons/lucide/chevron-right';
  import type { ListDone } from '../../lib/ipc/bindings';
  import { tickTask, untickTask } from './task-actions';
  import TaskRow from './TaskRow.svelte';

  let { done }: { done: ListDone } = $props();

  let open = $state(false);
  const label = $derived(done.mode === 'folded' ? 'Done' : 'Done earlier');
</script>

{#if done.shown.length > 0 || done.folded.length > 0}
  <section aria-label="Done" class="mt-3">
    {#each done.shown as task (task.id)}
      <TaskRow {task} ontick={tickTask} onuntick={untickTask} />
    {/each}
    {#if done.folded.length > 0}
      <button
        type="button"
        aria-expanded={open}
        onclick={() => (open = !open)}
        class="flex w-full cursor-pointer items-center gap-1.5 rounded-sm px-2 py-1.5 text-left text-xs text-faint transition-colors
          duration-(--dur) ease-out hover:bg-hover hover:text-muted"
      >
        {#if open}<ChevronDown class="size-3.5" />{:else}<ChevronRight class="size-3.5" />{/if}
        {label} · <span class="tabular-nums">{done.folded.length}</span>
      </button>
      {#if open}
        {#each done.folded as task (task.id)}
          <TaskRow {task} ontick={tickTask} onuntick={untickTask} />
        {/each}
      {/if}
    {/if}
  </section>
{/if}
