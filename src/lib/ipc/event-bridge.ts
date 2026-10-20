import type { QueryClient } from '@tanstack/svelte-query';
import { events } from './bindings';

/** One place where Rust events become cache invalidations. Returns the unlisten function. */
export async function startEventBridge(client: QueryClient): Promise<() => void> {
  return events.settingsChangedEvent.listen(() => {
    void client.invalidateQueries({ queryKey: ['settings'] });
  });
}
