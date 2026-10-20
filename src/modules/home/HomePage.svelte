<!-- HomePage (H1): greeting card, clock tile and the cards of enabled Modules (given the shell's context). -->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import Sparkles from '~icons/lucide/sparkles';
  import { readCore } from '../../lib/settings/core';
  import { appInfoQuery, settingsQuery } from '../../lib/settings/queries';
  import { clock } from '../../lib/time/clock.svelte';
  import Card from '../../lib/ui/Card.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import { homeCards } from '../../shell/enabled-modules';
  import { useEnabledModules } from '../../shell/enabled.svelte';
  import { moduleContext } from '../../shell/module-context';
  import ClockTile from './ClockTile.svelte';
  import GreetingCard from './GreetingCard.svelte';
  import { homeName } from './greeting-name';

  const settings = createQuery(() => settingsQuery);
  const info = createQuery(() => appInfoQuery);
  const core = $derived(readCore(settings.data?.values));
  const name = $derived(homeName(core.displayName, info.data?.os_first_name));

  $effect(() => clock.subscribe());

  const enabled = useEnabledModules();
  const cards = $derived(homeCards(enabled.list));
</script>

<div class="selectable h-full overflow-y-auto p-(--pad)">
  <PageHeader title="Home" />
  <div class="mx-auto flex max-w-3xl flex-col gap-(--gap)">
    <div class="grid grid-cols-1 items-stretch gap-(--gap) min-[720px]:grid-cols-[1.4fr_1fr]">
      <GreetingCard now={clock.now} {name} dateFormat={core.dateFormat} />
      <ClockTile now={clock.now} format={core.timeFormat} />
    </div>
    {#each cards as c (c.id)}
      {#await c.load() then card}
        <card.default ctx={moduleContext} />
      {/await}
    {:else}
      <Card>
        <EmptyState
          icon={Sparkles}
          title="Your Modules will live here"
          description="Cards from your Modules will show up on Home."
        />
      </Card>
    {/each}
  </div>
</div>
