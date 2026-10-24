import type { QueryClient } from '@tanstack/svelte-query';
import { events } from './bindings';

/** The cache key every Todo query starts with (see `todoKeys` in the Todo Module). */
const TODO_KEY = ['todo'] as const;

/** The cache key every Journal query starts with (see `journalKeys` in the Journal Module). */
const JOURNAL_KEY = ['journal'] as const;

/** One place where Rust events become cache invalidations. Returns the unlisten function. */
export async function startEventBridge(client: QueryClient): Promise<() => void> {
  const unlistenSettings = await events.settingsChangedEvent.listen(() => {
    // A Preference can change what any view shows (Todo's Done Tasks in Lists, the Week start), so
    // every cached query is refreshed; only the ones on screen refetch now.
    void client.invalidateQueries();
  });
  // The Todo widget wrote while the app was alive: what Todo shows is stale.
  let unlistenTodo: () => void;
  let unlistenJournal: () => void;
  try {
    unlistenTodo = await events.todoChangedEvent.listen(() => {
      void client.invalidateQueries({ queryKey: TODO_KEY });
    });
  } catch (error) {
    // Do not leave the first listener behind when the second one cannot be registered.
    unlistenSettings();
    throw error;
  }
  try {
    // Sync changed Journal data from another device: views refresh in place, without a toast.
    unlistenJournal = await events.journalChangedEvent.listen(() => {
      void client.invalidateQueries({ queryKey: JOURNAL_KEY });
    });
  } catch (error) {
    unlistenSettings();
    unlistenTodo();
    throw error;
  }
  // The Todo widget can also write while the app is stopped or its webview is paused, and no
  // event arrives then: coming back to the app refetches Todo.
  const onVisible = () => {
    if (!document.hidden) void client.invalidateQueries({ queryKey: TODO_KEY });
  };
  document.addEventListener('visibilitychange', onVisible);
  return () => {
    unlistenSettings();
    unlistenTodo();
    unlistenJournal();
    document.removeEventListener('visibilitychange', onVisible);
  };
}
