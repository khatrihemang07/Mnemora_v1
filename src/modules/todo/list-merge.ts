import type { TodoListsMergedEvent } from '../../lib/ipc/bindings';
import { listView, sameView, type TodoView } from './views';

/** "Groceries merged with Groceries from Pixel 8": the toast when the List you were on merged away. */
export function mergedMessage(e: TodoListsMergedEvent): string {
  const other = e.device ? `${e.into_name} from ${e.device}` : e.into_name;
  return `${e.from_name} merged with ${other}`;
}

/**
 * Where Todo goes when two Lists with one name merge: someone on the List that went away moves to
 * the one that was kept and is told; anyone else stays where they are.
 */
export function followMerge(
  view: TodoView,
  e: TodoListsMergedEvent,
): { view: TodoView; message: string } | null {
  if (!sameView(view, listView(e.from_id))) return null;
  return { view: listView(e.into_id), message: mergedMessage(e) };
}
