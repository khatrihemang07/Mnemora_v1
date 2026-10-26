<!-- The activity line above an answer: one live line while the Run works, then a collapsed
     "Completed N steps" that expands into the plain-language steps. Spinners stop under reduced motion. -->
<script lang="ts">
  import Check from '~icons/lucide/check';
  import ChevronDown from '~icons/lucide/chevron-down';
  import ChevronRight from '~icons/lucide/chevron-right';
  import LoaderCircle from '~icons/lucide/loader-circle';
  import Button from '../../lib/ui/Button.svelte';
  import { type ActivityStep, completedLabel, runningLabel } from './activity';

  interface Props {
    steps: readonly ActivityStep[];
    /** Whether the Run is still working; the line then shows progress, not the steps. */
    running: boolean;
  }

  let { steps, running }: Props = $props();
  let expanded = $state(false);
</script>

{#if running}
  <!-- Not a live region: the Run's end is announced once, by AiPage (announce.ts); this line is visual. -->
  <p class="flex items-center gap-2 text-sm text-muted">
    <LoaderCircle class="size-4 motion-safe:animate-spin" aria-hidden="true" />
    <span>{runningLabel(steps.length)}</span>
  </p>
{:else if steps.length > 0}
  <div class="flex flex-col gap-2 text-sm text-muted">
    <div>
      <Button
        variant="ghost"
        size="sm"
        aria-expanded={expanded}
        onclick={() => (expanded = !expanded)}
      >
        <Check class="size-4" aria-hidden="true" />
        <span>{completedLabel(steps.length)}</span>
        {#if expanded}
          <ChevronDown class="size-4" aria-hidden="true" />
        {:else}
          <ChevronRight class="size-4" aria-hidden="true" />
        {/if}
      </Button>
    </div>
    {#if expanded}
      <ol class="ml-3 flex flex-col gap-1 border-l border-line pl-3">
        {#each steps as step, index (index)}
          <li class={step.isError ? 'text-danger' : ''}>{step.summary ?? step.name}</li>
        {/each}
      </ol>
    {/if}
  </div>
{/if}
