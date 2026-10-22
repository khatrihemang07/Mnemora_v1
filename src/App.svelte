<script lang="ts">
  import { QueryClientProvider } from '@tanstack/svelte-query';
  import { onMount } from 'svelte';
  import { startEventBridge } from './lib/ipc/event-bridge';
  import { queryClient } from './lib/ipc/query-client';
  import { installBackButton } from './lib/router';
  import { startAppLinkBridge } from './shell/deep-links';
  import AppShell from './shell/AppShell.svelte';

  // The page is held back until the link that started the app (a Todo widget tap) has been
  // followed, so a cold open lands on Todo and never flashes Home. A slow or failing read must not
  // keep the app blank, hence the cap.
  const LINK_WAIT_MS = 1500;
  let ready = $state(false);

  onMount(() => {
    const stop = startEventBridge(queryClient);
    // Links that opened the app (the Todo widget's taps), including the one that started it.
    const stopLinks = startAppLinkBridge();
    const open = () => (ready = true);
    void Promise.race([stopLinks, new Promise((r) => setTimeout(r, LINK_WAIT_MS))]).then(open, open);
    void installBackButton();
    return () => {
      void stop.then((unlisten) => unlisten());
      void stopLinks.then((unlisten) => unlisten());
    };
  });
</script>

<QueryClientProvider client={queryClient}>
  {#if ready}
    <AppShell />
  {/if}
</QueryClientProvider>
