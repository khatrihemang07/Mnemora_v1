import { queryOptions } from '@tanstack/svelte-query';
import {
  type AllView,
  commands,
  type DeleteChoice,
  type InboxView,
  type ListDeletion,
  type ListView,
  type Task,
  type TodoList,
} from '../../lib/ipc/bindings';
import { queryClient } from '../../lib/ipc/query-client';
import { unwrap } from '../../lib/ipc/unwrap';
import { withoutTask, withoutTaskInAll } from './tasks';

export const todoKeys = {
  all: ['todo'] as const,
  inbox: ['todo', 'inbox'] as const,
  allView: ['todo', 'all'] as const,
  lists: ['todo', 'lists'] as const,
  list: (id: string) => ['todo', 'list', id] as const,
};

/** The Inbox: open Tasks that belong to no List, in order. */
export const inboxQuery = queryOptions({
  queryKey: todoKeys.inbox,
  queryFn: () => unwrap(commands.todoInbox()),
});

/** The sidebar: the Inbox count and every List with its counts. */
export const listsQuery = queryOptions({
  queryKey: todoKeys.lists,
  queryFn: () => unwrap(commands.todoLists()),
});

/** All open Tasks grouped by List, the Inbox first. */
export const allQuery = queryOptions({
  queryKey: todoKeys.allView,
  queryFn: () => unwrap(commands.todoAll()),
});

/** One List and its open Tasks. */
export const listQuery = (id: string) =>
  queryOptions({
    queryKey: todoKeys.list(id),
    queryFn: () => unwrap(commands.todoList(id)),
  });

export function invalidateTodo(): Promise<void> {
  return queryClient.invalidateQueries({ queryKey: todoKeys.all });
}

/** Adds a Task at the bottom of a List, or of the Inbox when `listId` is null. */
export function createTask(title: string, listId: string | null = null): Promise<Task> {
  return unwrap(commands.todoTaskCreate(title, listId));
}

export function completeTask(id: string): Promise<Task> {
  return unwrap(commands.todoTaskComplete(id));
}

export function reopenTask(id: string): Promise<Task> {
  return unwrap(commands.todoTaskReopen(id));
}

/** Puts a Task in a List, or in the Inbox when `listId` is null. */
export function moveTask(id: string, listId: string | null): Promise<Task> {
  return unwrap(commands.todoTaskMove(id, listId));
}

export function createList(name: string): Promise<TodoList> {
  return unwrap(commands.todoListCreate(name));
}

export function renameList(id: string, name: string): Promise<TodoList> {
  return unwrap(commands.todoListRename(id, name));
}

export function setListEmoji(id: string, emoji: string | null): Promise<TodoList> {
  return unwrap(commands.todoListSetEmoji(id, emoji));
}

export function deleteList(id: string, choice: DeleteChoice): Promise<ListDeletion> {
  return unwrap(commands.todoListDelete(id, choice));
}

export function restoreList(deletion: ListDeletion): Promise<TodoList> {
  return unwrap(commands.todoListRestore(deletion));
}

/** Takes a Task off every cached page at once, before Rust has answered. */
export function removeCachedTask(id: string) {
  queryClient.setQueryData<InboxView>(todoKeys.inbox, (v) => v && withoutTask(v, id));
  queryClient.setQueryData<AllView>(todoKeys.allView, (v) => v && withoutTaskInAll(v, id));
  queryClient.setQueriesData<ListView>(
    { queryKey: ['todo', 'list'] },
    (v) => v && withoutTask(v, id),
  );
}
