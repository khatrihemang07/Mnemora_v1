import { commands, type SavedQuestion } from '../../lib/ipc/bindings';
import { queryClient } from '../../lib/ipc/query-client';
import { unwrap } from '../../lib/ipc/unwrap';
import { notifyUndo } from '../../lib/ui/toast';
import { aiKeys } from './queries';
import { type MoveDirection, moveNeighbours } from './saved-order';

const refresh = () => queryClient.invalidateQueries({ queryKey: aiKeys.saved });

/** Saves a question as a Saved question at the end of the list. */
export async function saveQuestion(text: string): Promise<SavedQuestion> {
  const saved = await unwrap(commands.aiSavedCreate(text));
  await refresh();
  return saved;
}

/** Changes a Saved question's text. */
export async function renameSaved(id: string, text: string): Promise<void> {
  await unwrap(commands.aiSavedUpdate(id, text));
  await refresh();
}

/** Moves a Saved question one place up or down; nothing happens at the ends. */
export async function moveSaved(
  list: readonly SavedQuestion[],
  id: string,
  direction: MoveDirection,
): Promise<void> {
  const next = moveNeighbours(
    list.map((q) => q.id),
    id,
    direction,
  );
  if (!next) return;
  await unwrap(commands.aiSavedReorder(id, next.afterId, next.beforeId));
  await refresh();
}

/** Deletes a Saved question at once, with an Undo toast that puts it back in its place. */
export async function deleteSaved(saved: SavedQuestion): Promise<void> {
  await unwrap(commands.aiSavedDelete(saved.id));
  await refresh();
  notifyUndo('Saved question deleted', () => {
    void unwrap(commands.aiSavedRestore(saved.id)).then(refresh);
  });
}
