import type { DeleteChoice, TodoList } from '../../lib/ipc/bindings';
import { quoted } from './tasks';

/** Longest List name; Rust enforces it too. */
export const MAX_LIST_NAME = 60;

const tasks = (n: number) => `${n} ${n === 1 ? 'Task' : 'Tasks'}`;

/** The Undo toast after deleting a List: says what became of its Tasks. */
export function deletedListMessage(
  list: TodoList,
  taskCount: number,
  choice: DeleteChoice,
  targetName?: string,
): string {
  const name = quoted(list.name);
  if (taskCount === 0) return `Deleted ${name}`;
  switch (choice.kind) {
    case 'delete_tasks':
      return `Deleted ${name} and ${tasks(taskCount)}`;
    case 'move_to_inbox':
      return `Deleted ${name}; ${tasks(taskCount)} moved to the Inbox`;
    case 'move_to_list':
      return `Deleted ${name}; ${tasks(taskCount)} moved to ${quoted(targetName ?? 'another List')}`;
  }
}

/** The Undo toast after moving a Task; no List is the Inbox. */
export const movedMessage = (list: TodoList | null): string =>
  list ? `Moved to ${quoted(list.name)}` : 'Moved to the Inbox';
