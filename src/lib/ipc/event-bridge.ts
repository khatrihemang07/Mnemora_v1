import type { QueryClient } from '@tanstack/svelte-query';
import { events } from './bindings';

/** The cache key every Todo query starts with (see `todoKeys` in the Todo Module). */
const TODO_KEY = ['todo'] as const;

/** One place where Rust events become cache invalidations. Returns the unlisten function. */
export async function startEventBridge(client: QueryClient): Promise<() => void> {
  const unlistenSettings = await events.settingsChangedEvent.listen(() => {
    // A Preference can change what any view shows (Todo's Done Tasks in Lists, the Week start), so
    // every cached query is refreshed; only the ones on screen refetch now.
    void client.invalidateQueries();
  });
  // The Todo widget wrote while the app was alive: what Todo shows is stale.
  const unlistenTodo = await events.todoChangedEvent.listen(() => {
    void client.invalidateQueries({ queryKey: TODO_KEY });
  });
  // The Todo widget can also write while the app is stopped or its webview is paused, and no
  // event arrives then: coming back to the app refetches Todo.
  const onVisible = () => {
    if (!document.hidden) void client.invalidateQueries({ queryKey: TODO_KEY });
  };
  document.addEventListener('visibilitychange', onVisible);
  return () => {
    unlistenSettings();
    unlistenTodo();
    document.removeEventListener('visibilitychange', onVisible);
  };
}
