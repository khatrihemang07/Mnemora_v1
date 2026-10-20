import { QueryClient } from '@tanstack/svelte-query';

/** `networkMode: 'always'` because IPC is local; "offline" must never pause a query. */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { networkMode: 'always', refetchOnWindowFocus: false, retry: 1 },
    mutations: { networkMode: 'always' },
  },
});
