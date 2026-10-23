import type { TodoTaskDeletedEvent } from '../../lib/ipc/bindings';

/** "‘Plan weekend hike’ was deleted on Pixel 8": the toast when the Task in the sheet is deleted elsewhere. */
export function remoteDeletedMessage(e: TodoTaskDeletedEvent): string {
  return `‘${e.title}’ was deleted on ${e.device ?? 'another device'}`;
}

/** What to do when a Task is deleted on another device: only the Task open in the sheet is told. */
export function followRemoteDelete(
  openTaskId: string | null,
  e: TodoTaskDeletedEvent,
): { message: string } | null {
  return openTaskId !== null && openTaskId === e.task_id
    ? { message: remoteDeletedMessage(e) }
    : null;
}

/** Tasks the follower has already told the person about, so the sheet does not also say "no longer exists". */
const told = new Set<string>();
export const remoteDeletes = {
  mark: (id: string) => void told.add(id),
  has: (id: string) => told.has(id),
  clear: (id: string) => void told.delete(id),
};
