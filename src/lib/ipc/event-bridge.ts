import type { QueryClient } from '@tanstack/svelte-query';
import { events } from './bindings';

/** One place where Rust events become cache invalidations. Returns the unlisten function. */
export async function startEventBridge(client: QueryClient): Promise<() => void> {
  return events.settingsChangedEvent.listen(() => {
    // A Preference can change what any view shows (Todo's Done Tasks in Lists, the Week start), so
    // every cached query is refreshed; only the ones on screen refetch now.
    void client.invalidateQueries();
  });
}
