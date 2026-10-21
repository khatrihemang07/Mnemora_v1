import type { DeleteChoice, ListEntry } from '../../lib/ipc/bindings';
import { notifyError, notifyUndo } from '../../lib/ui/toast';
import { deletedListMessage } from './lists';
import { todoNav } from './nav.svelte';
import { createList, deleteList, invalidateTodo, restoreList } from './queries';
import { listView, sameView, viewAfterListGone } from './views';

/** Creates a List and opens it. Throws Rust's refusal (a taken or invalid name) for the field to show. */
export async function createListAndOpen(name: string): Promise<void> {
  const list = await createList(name);
  await invalidateTodo();
  todoNav.go(listView(list.id));
}

/**
 * Deletes a List and offers Undo. Its Tasks are deleted with it or moved, as `choice` says; the
 * toast says which. If the List was open, Todo goes to the Inbox, and Undo brings the List back
 * and reopens it.
 */
export async function deleteListWithUndo(
  entry: ListEntry,
  choice: DeleteChoice,
  targetName?: string,
): Promise<void> {
  const { list } = entry;
  const wasOpen = sameView(todoNav.view, listView(list.id));
  try {
    const deletion = await deleteList(list.id, choice);
    todoNav.go(viewAfterListGone(todoNav.view, list.id));
    void invalidateTodo();
    notifyUndo(deletedListMessage(list, entry.task_count, choice, targetName), () => {
      restoreList(deletion)
        .then(() => {
          if (wasOpen) todoNav.go(listView(list.id));
        })
        .catch((e) => notifyError(e instanceof Error ? e.message : "Couldn't restore the List"))
        .finally(() => void invalidateTodo());
    });
  } catch {
    void invalidateTodo();
    notifyError("Couldn't delete the List");
  }
}
