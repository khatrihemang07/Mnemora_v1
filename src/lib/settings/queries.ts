import { queryOptions } from '@tanstack/svelte-query';
import { commands } from '../ipc/bindings';
import { unwrap } from '../ipc/unwrap';

export const settingsQuery = queryOptions({
  queryKey: ['settings'],
  queryFn: () => unwrap(commands.settingsGetAll()),
});

export const appInfoQuery = queryOptions({
  queryKey: ['app-info'],
  queryFn: () => commands.appInfo(),
  staleTime: Number.POSITIVE_INFINITY,
});

export const deviceQuery = queryOptions({
  queryKey: ['device'],
  queryFn: () => commands.deviceGet(),
  staleTime: Number.POSITIVE_INFINITY,
});
