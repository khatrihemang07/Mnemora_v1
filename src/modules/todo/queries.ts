import { queryOptions } from '@tanstack/svelte-query';
import {
  type AllView,
  commands,
  type DeleteChoice,
  type DueChange,
  type InboxView,
  type ListDeletion,
  type ListView,
  type OverdueMove,
  type Task,
  type TodayView,
  type TodoList,
  type UpcomingView,
} from '../../lib/ipc/bindings';
import { queryClient } from '../../lib/ipc/query-client';
import { unwrap } from '../../lib/ipc/unwrap';
import { todoClock } from './clock.svelte';
import { withoutTask, withoutTaskInAll, withoutTaskInToday, withoutTaskInUpcoming } from './tasks';

export const todoKeys = {
  all: ['todo'] as const,
  inbox: ['todo', 'inbox'] as const,
  allView: ['todo', 'all'] as const,
  lists: ['todo', 'lists'] as const,
  list: (id: string) => ['todo', 'list', id] as const,
  /** Keyed by the day, so the view fetches afresh when midnight passes. */
  today: (day: string) => ['todo', 'today', day] as const,
  upcoming: (day: string) => ['todo', 'upcoming', day] as const,
  dayCounts: (from: string, to: string, exclude: string | null) =>
    ['todo', 'day-counts', from, to, exclude] as const,
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

/** Today: Overdue, the day's Tasks and what is done. `day` is today's date, for the key. */
export const todayQuery = (day: string) =>
  queryOptions({
    queryKey: todoKeys.today(day),
    queryFn: () => unwrap(commands.todoToday(todoClock.local())),
  });

/** Upcoming: Overdue, then each later day that has Tasks. */
export const upcomingQuery = (day: string) =>
  queryOptions({
    queryKey: todoKeys.upcoming(day),
    queryFn: () => unwrap(commands.todoUpcoming(todoClock.local())),
  });

/** Open Tasks per date from `from` to `to`, leaving out `exclude`: the When picker's dots. */
export const dayCountsQuery = (from: string, to: string, exclude: string | null) =>
  queryOptions({
    queryKey: todoKeys.dayCounts(from, to, exclude),
    queryFn: () => unwrap(commands.todoDayCounts(from, to, exclude)),
  });

export function invalidateTodo(): Promise<void> {
  return queryClient.invalidateQueries({ queryKey: todoKeys.all });
}

/**
 * Adds a Task at the bottom of a List, or of the Inbox when `listId` is null. With `date` it is
 * due that day (Today and an Upcoming day add this way).
 */
export function createTask(
  title: string,
  listId: string | null = null,
  date: string | null = null,
): Promise<Task> {
  const when = date === null ? null : { date, time: null, local: todoClock.local() };
  return unwrap(commands.todoTaskCreate(title, listId, when));
}

/** Sets a Task's date and time, or clears them. Rust refuses past dates. */
export function setWhen(id: string, date: string | null, time: string | null): Promise<Task> {
  return unwrap(commands.todoTaskSetWhen(id, date, time, todoClock.local()));
}

/** Puts dates and times back as they were (Undo), past dates included. */
export function restoreWhen(changes: DueChange[]): Promise<Task[]> {
  return unwrap(commands.todoTasksRestoreWhen(changes));
}

/** Moves every Overdue Task to `date`. */
export function moveOverdue(date: string): Promise<OverdueMove> {
  return unwrap(commands.todoTasksMoveOverdue(date, todoClock.local()));
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
  queryClient.setQueriesData<TodayView>(
    { queryKey: ['todo', 'today'] },
    (v) => v && withoutTaskInToday(v, id),
  );
  queryClient.setQueriesData<UpcomingView>(
    { queryKey: ['todo', 'upcoming'] },
    (v) => v && withoutTaskInUpcoming(v, id),
  );
}
