<script lang="ts">
  import { QueryClientProvider } from '@tanstack/svelte-query';
  import { onMount } from 'svelte';
  import { startEventBridge } from './lib/ipc/event-bridge';
  import { queryClient } from './lib/ipc/query-client';
  import { installBackButton } from './lib/router';
  import { startAppLinkBridge } from './shell/deep-links';
  import AppShell from './shell/AppShell.svelte';

  onMount(() => {
    const stop = startEventBridge(queryClient);
    // Links that opened the app (the Todo widget's taps), including the one that started it.
    const stopLinks = startAppLinkBridge();
    void installBackButton();
    return () => {
      void stop.then((unlisten) => unlisten());
      void stopLinks.then((unlisten) => unlisten());
    };
  });
</script>

<QueryClientProvider client={queryClient}>
  <AppShell />
</QueryClientProvider>
