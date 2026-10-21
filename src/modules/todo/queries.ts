import { queryOptions } from '@tanstack/svelte-query';
import { commands, type InboxView, type Task } from '../../lib/ipc/bindings';
import { queryClient } from '../../lib/ipc/query-client';
import { unwrap } from '../../lib/ipc/unwrap';
import { withoutTask } from './tasks';

export const todoKeys = {
  all: ['todo'] as const,
  inbox: ['todo', 'inbox'] as const,
};

/** The Inbox: open Tasks that belong to no List, in order. */
export const inboxQuery = queryOptions({
  queryKey: todoKeys.inbox,
  queryFn: () => unwrap(commands.todoInbox()),
});

export function invalidateTodo(): Promise<void> {
  return queryClient.invalidateQueries({ queryKey: todoKeys.all });
}

export function createTask(title: string): Promise<Task> {
  return unwrap(commands.todoTaskCreate(title));
}

export function completeTask(id: string): Promise<Task> {
  return unwrap(commands.todoTaskComplete(id));
}

export function reopenTask(id: string): Promise<Task> {
  return unwrap(commands.todoTaskReopen(id));
}

/** Takes a Task off the cached Inbox at once, before Rust has answered. */
export function removeCachedTask(id: string) {
  queryClient.setQueryData<InboxView>(todoKeys.inbox, (view) => view && withoutTask(view, id));
}
