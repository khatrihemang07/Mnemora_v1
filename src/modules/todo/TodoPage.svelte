<!--
  TodoPage: Todo's single route. The view (Inbox, Today, Upcoming, All, Done or a List) is held in
  Todo's own state, set from the sidebar tree; this shows the one that is current.
-->
<script lang="ts">
  import AllPage from './AllPage.svelte';
  import InboxPage from './InboxPage.svelte';
  import ListPage from './ListPage.svelte';
  import { todoNav } from './nav.svelte';
  import SoonPage from './SoonPage.svelte';

  const view = $derived(todoNav.view);
</script>

<div class="selectable h-full overflow-y-auto p-(--pad)">
  {#if view.kind === 'list'}
    {#key view.id}<ListPage id={view.id} />{/key}
  {:else if view.id === 'inbox'}
    <InboxPage />
  {:else if view.id === 'all'}
    <AllPage />
  {:else}
    <SoonPage id={view.id} />
  {/if}
</div>
