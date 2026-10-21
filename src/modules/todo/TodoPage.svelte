<!--
  TodoPage: Todo's single route. The view (Inbox, Today, Upcoming, All, Done or a List) is held in
  Todo's own state, set from the sidebar tree; this shows the one that is current.
-->
<script lang="ts">
  import { onDestroy } from 'svelte';
  import { clock } from '../../lib/time/clock.svelte';
  import AllPage from './AllPage.svelte';
  import DonePage from './DonePage.svelte';
  import InboxPage from './InboxPage.svelte';
  import ListPage from './ListPage.svelte';
  import { todoNav } from './nav.svelte';
  import { followCoreSettings } from './prefs.svelte';
  import TodayPage from './TodayPage.svelte';
  import UpcomingPage from './UpcomingPage.svelte';

  // The shared clock ticks only while someone is subscribed; the views read the day and the minute
  // from it, so midnight rolls them over without any polling of ours.
  onDestroy(clock.subscribe());
  followCoreSettings();

  const view = $derived(todoNav.view);
</script>

<div class="selectable h-full overflow-y-auto p-(--pad)">
  {#if view.kind === 'list'}
    {#key view.id}<ListPage id={view.id} />{/key}
  {:else if view.id === 'inbox'}
    <InboxPage />
  {:else if view.id === 'all'}
    <AllPage />
  {:else if view.id === 'today'}
    <TodayPage />
  {:else if view.id === 'upcoming'}
    <UpcomingPage />
  {:else}
    <DonePage />
  {/if}
</div>
