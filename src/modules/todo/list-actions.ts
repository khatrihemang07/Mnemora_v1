import type { DeleteChoice, ListEntry, ListSort } from '../../lib/ipc/bindings';
import { queryClient } from '../../lib/ipc/query-client';
import type { MenuItem } from '../../lib/ui/Menu.svelte';
import { notifyError, notifyUndo } from '../../lib/ui/toast';
import { LIST_SORT_OPTIONS, LISTS_SORT_PREF, sortMenuLabel } from './arrange';
import { deletedListMessage } from './lists';
import { todoNav } from './nav.svelte';
import {
  createList,
  deleteList,
  invalidateTodo,
  reorderList,
  restoreList,
  savePreference,
  todoKeys,
} from './queries';
import { listView, sameView, viewAfterListGone } from './views';

/**
 * The "Sort Lists by" choice menu, the sidebar's and the phone Lists sheet's: the current sort is
 * ticked and picking one saves it as a Preference. `canDrag` says whether Manual is a drag order
 * there (the sheet has no drag, so it reads plain "Manual").
 */
export function listSortItems(sortedBy: ListSort, canDrag: boolean): MenuItem[] {
  return LIST_SORT_OPTIONS.map((o) => ({
    label: canDrag ? sortMenuLabel(o) : o.label,
    checked: sortedBy === o.value,
    onselect: () => {
      void savePreference(LISTS_SORT_PREF, o.value).catch(() =>
        notifyError("Couldn't sort the Lists"),
      );
    },
  }));
}

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
    // Forget the List's page, so its "gone" error cannot greet it when Undo reopens it.
    queryClient.removeQueries({ queryKey: todoKeys.listPages(list.id) });
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

/** Drags a List to a new place in the sidebar, between two neighbours. */
export async function placeList(
  id: string,
  before: string | null,
  after: string | null,
): Promise<void> {
  try {
    await reorderList(id, before, after);
  } catch {
    notifyError("Couldn't move the List");
  } finally {
    void invalidateTodo();
  }
}
