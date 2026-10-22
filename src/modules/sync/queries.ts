import { queryOptions } from '@tanstack/svelte-query';
import { commands, events } from '../../lib/ipc/bindings';
import { queryClient } from '../../lib/ipc/query-client';
import { unwrap } from '../../lib/ipc/unwrap';
import { deviceQuery } from '../../lib/settings/queries';

/** Sync's state for this device: on or off, name, options, and how many devices are paired. */
export const syncQuery = queryOptions({
  queryKey: ['sync'],
  queryFn: () => unwrap(commands.syncGet()),
  staleTime: Number.POSITIVE_INFINITY,
});

/** Asks for fresh values after a change; the device file also holds Sync's on/off. */
export async function refreshSync() {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: syncQuery.queryKey }),
    queryClient.invalidateQueries({ queryKey: deviceQuery.queryKey }),
  ]);
}

/** This device (addresses, port) and the paired devices. */
export const devicesQuery = queryOptions({
  queryKey: ['sync', 'devices'],
  queryFn: () => unwrap(commands.syncDevices()),
  staleTime: Number.POSITIVE_INFINITY,
});

/** Reads the Devices tab again whenever a paired device's status or progress changes. Returns what stops listening. */
export async function watchDevices(): Promise<() => void> {
  return events.syncPeersEvent.listen(() => {
    void queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey });
  });
}
