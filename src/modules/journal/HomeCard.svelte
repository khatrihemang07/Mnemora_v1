<!--
  HomeCard: "Today in your Journal" on Home. The latest Entry written today as a plain-text quote in
  the display serif, today's count, and a Write button that opens the Journal on the focused composer.
  An empty day says "Nothing written today". Refreshes when Journal writes invalidate its query.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import NotebookPen from '~icons/lucide/notebook-pen';
  import PenLine from '~icons/lucide/pen-line';
  import type { HomeCardProps } from '../../lib/router';
  import { clock } from '../../lib/time/clock.svelte';
  import Button from '../../lib/ui/Button.svelte';
  import Card from '../../lib/ui/Card.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import { localDateString } from './dates';
  import { requestCompose } from './launcher.svelte';
  import { todayQuery } from './queries';
  import { todayCardText } from './today-card';

  let { ctx }: HomeCardProps = $props();

  // A string, so it changes only at midnight even though the clock ticks every second.
  const date = $derived(localDateString(clock.now));
  const summary = createQuery(() => todayQuery(date));
  const text = $derived(summary.data ? todayCardText(summary.data) : null);

  function write() {
    requestCompose();
    ctx.navigate('/journal');
  }
</script>

<Card>
  <div class="flex items-center gap-2.5">
    <span
      class="grid size-8 place-items-center rounded-full bg-[color-mix(in_oklch,var(--hue-journal)_14%,transparent)] text-(--hue-journal)"
    >
      <NotebookPen class="size-4" />
    </span>
    <h2 class="font-semibold text-ink">Journal</h2>
    <Button size="sm" variant="primary" class="ml-auto" onclick={write}>
      <PenLine class="size-3.5" /> Write
    </Button>
  </div>
  {#if summary.isError}
    <div class="mt-3"><ErrorBanner error={summary.error} onretry={() => summary.refetch()} /></div>
  {:else if text}
    {#if text.quote}
      <p
        class="mt-3 line-clamp-3 font-display text-lg leading-snug text-ink"
        style="font-weight: var(--display-weight)"
      >
        {text.quote}
      </p>
    {/if}
    <p class="{text.quote ? 'mt-2 text-sm' : 'mt-3'} text-muted">{text.meta}</p>
  {:else}
    <Skeleton class="mt-3 h-12 w-full" />
  {/if}
</Card>
