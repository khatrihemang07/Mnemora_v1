<!-- HomePage (H1): greeting card, clock tile and an invitation for future Modules. -->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import Sparkles from '~icons/lucide/sparkles';
  import { readCore } from '../../lib/settings/core';
  import { appInfoQuery, settingsQuery } from '../../lib/settings/queries';
  import { clock } from '../../lib/time/clock.svelte';
  import Card from '../../lib/ui/Card.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import ClockTile from './ClockTile.svelte';
  import GreetingCard from './GreetingCard.svelte';
  import { homeName } from './greeting-name';

  const settings = createQuery(() => settingsQuery);
  const info = createQuery(() => appInfoQuery);
  const core = $derived(readCore(settings.data?.values));
  const name = $derived(homeName(core.displayName, info.data?.os_first_name));

  $effect(() => clock.subscribe());

  const soon = ['Journal', 'Todo', 'AI'];
</script>

<div class="selectable h-full overflow-y-auto p-(--pad)">
  <PageHeader title="Home" />
  <div class="mx-auto flex max-w-3xl flex-col gap-(--gap)">
    <div class="grid grid-cols-1 items-stretch gap-(--gap) min-[720px]:grid-cols-[1.4fr_1fr]">
      <GreetingCard now={clock.now} {name} dateFormat={core.dateFormat} />
      <ClockTile now={clock.now} format={core.timeFormat} />
    </div>
    <Card>
      <EmptyState
        icon={Sparkles}
        title="Your Modules will live here"
        description="Journal, Todo and AI are on the way."
      >
        {#snippet action()}
          <ul class="flex flex-wrap justify-center gap-2">
            {#each soon as m (m)}
              <li class="rounded-full border border-line bg-surface-2 px-3 py-1 text-sm text-muted">
                {m} <span class="text-faint">· soon</span>
              </li>
            {/each}
          </ul>
        {/snippet}
      </EmptyState>
    </Card>
  </div>
</div>
