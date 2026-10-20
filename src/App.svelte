<script lang="ts">
  import { QueryClientProvider } from '@tanstack/svelte-query';
  import { onMount } from 'svelte';
  import { startEventBridge } from './lib/ipc/event-bridge';
  import { queryClient } from './lib/ipc/query-client';
  import { installBackButton } from './lib/router';
  import AppShell from './shell/AppShell.svelte';

  onMount(() => {
    const stop = startEventBridge(queryClient);
    void installBackButton();
    return () => void stop.then((unlisten) => unlisten());
  });
</script>

<QueryClientProvider client={queryClient}>
  <AppShell />
</QueryClientProvider>
