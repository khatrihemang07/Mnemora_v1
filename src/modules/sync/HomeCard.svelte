<!--
  HomeCard: Sync on Home, only while something needs the person: Conflicts wait, sync is paused, or
  a paired device has not been seen for 14 days (B10). Tile, "Sync", one line ("2 conflicts to
  review · Pixel 8 synced 2 min ago") and Review. All is fine: nothing is drawn, so Home stays calm.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import RadioTower from '~icons/lucide/radio-tower';
  import type { HomeCardProps } from '../../lib/router';
  import { clock } from '../../lib/time/clock.svelte';
  import Button from '../../lib/ui/Button.svelte';
  import Card from '../../lib/ui/Card.svelte';
  import { homeCardLine, reviewPath } from './attention';
  import { attentionQuery } from './queries';

  let { ctx }: HomeCardProps = $props();

  // Ticks the "synced 2 min ago" along.
  $effect(() => clock.subscribe());

  const attention = createQuery(() => attentionQuery);
  const a = $derived(attention.data?.needed ? attention.data : null);
</script>

{#if a}
  <section data-home-sync>
    <Card>
      <div class="flex items-center gap-2.5">
        <span class="grid size-8 shrink-0 place-items-center rounded-full bg-hue-sync-soft text-hue-sync">
          <RadioTower class="size-4" />
        </span>
        <div class="min-w-0 flex-1">
          <h2 class="font-semibold text-ink">Sync</h2>
          <p class="text-sm text-muted">{homeCardLine(a, clock.now)}</p>
        </div>
        <Button size="sm" variant="secondary" onclick={() => ctx.navigate(reviewPath(a))}>
          Review
        </Button>
      </div>
    </Card>
  </section>
{/if}
