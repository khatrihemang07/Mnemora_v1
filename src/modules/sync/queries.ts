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

/** "Needs you": the Conflicts that wait. */
export const conflictsQuery = queryOptions({
  queryKey: ['sync', 'conflicts'],
  queryFn: () => unwrap(commands.syncConflicts()),
  staleTime: Number.POSITIVE_INFINITY,
});

/** One Conflict for Compare. */
export const conflictQuery = (id: string) =>
  queryOptions({
    queryKey: ['sync', 'conflict', id],
    queryFn: () => unwrap(commands.syncConflict(id)),
    staleTime: Number.POSITIVE_INFINITY,
  });

/** Reads Conflicts again when one is raised or settled (here or on another device). Returns what stops listening. */
export async function watchConflicts(): Promise<() => void> {
  return events.syncConflictsEvent.listen(() => {
    void queryClient.invalidateQueries({ queryKey: ['sync', 'conflicts'] });
    void queryClient.invalidateQueries({ queryKey: ['sync', 'conflict'] });
  });
}

/** What needs attention: the Home card, the sidebar badge and the palette hint all read this. */
export const attentionQuery = queryOptions({
  queryKey: ['sync', 'attention'],
  queryFn: () => unwrap(commands.syncAttention()),
  staleTime: Number.POSITIVE_INFINITY,
});

/**
 * Reads attention again whenever a Conflict is raised or settled or a device's status changes.
 * Returns what stops listening.
 */
export async function watchAttention(): Promise<() => void> {
  const refresh = () => void queryClient.invalidateQueries({ queryKey: attentionQuery.queryKey });
  const stops = await Promise.all([
    events.syncConflictsEvent.listen(refresh),
    events.syncPeersEvent.listen(refresh),
  ]);
  return () => {
    for (const stop of stops) stop();
  };
}

/** Starts `listen` and returns a cleanup that stops it, even if it is called before it finished. */
export function whileMounted(listen: () => Promise<() => void>): () => void {
  let stop: (() => void) | undefined;
  let gone = false;
  void listen().then((off) => {
    if (gone) off();
    else stop = off;
  });
  return () => {
    gone = true;
    stop?.();
  };
}
