import { infiniteQueryOptions, keepPreviousData, queryOptions } from '@tanstack/svelte-query';
import {
  type AllView,
  type Comment,
  type Completion,
  commands,
  type DeleteChoice,
  type DoneView,
  type DueChange,
  type GroupBy,
  type GroupKey,
  type InboxView,
  type ListDeletion,
  type ListView,
  type OverdueMove,
  type Parsed,
  type QuickEdit,
  type RepeatRule,
  type Task,
  type TaskSort,
  type TextRange,
  type TodayView,
  type TodoList,
  type UpcomingView,
} from '../../lib/ipc/bindings';
import { queryClient } from '../../lib/ipc/query-client';
import { unwrap } from '../../lib/ipc/unwrap';
import { todoClock } from './clock.svelte';
import { todoPrefs } from './prefs.svelte';
import { withoutTaskInGroups } from './tasks';

export const todoKeys = {
  all: ['todo'] as const,
  /** Keyed by the day: the Inbox groups by Due date, and Done-today depends on it. */
  inbox: (day: string) => ['todo', 'inbox', day] as const,
  allView: (day: string) => ['todo', 'all', day] as const,
  /** Keyed by the day: Today's count in the sidebar rolls over at midnight. */
  lists: (day: string) => ['todo', 'lists', day] as const,
  list: (id: string, day: string) => ['todo', 'list', id, day] as const,
  /** Every cached page of one List, whatever the day. */
  listPages: (id: string) => ['todo', 'list', id] as const,
  /** Keyed by the day, so the view fetches afresh when midnight passes. */
  today: (day: string) => ['todo', 'today', day] as const,
  upcoming: (day: string) => ['todo', 'upcoming', day] as const,
  /** Keyed by the day, so a Task done after midnight lands under the right heading. */
  done: (day: string) => ['todo', 'done', day] as const,
  dayCounts: (from: string, to: string, exclude: string | null) =>
    ['todo', 'day-counts', from, to, exclude] as const,
  task: (id: string) => ['todo', 'task', id] as const,
  comments: (id: string) => ['todo', 'comments', id] as const,
  history: (id: string) => ['todo', 'history', id] as const,
  search: (query: string) => ['todo', 'search', query] as const,
};

/** The Inbox: open Tasks with no List in groups, then undated Tasks in a List. */
export const inboxQuery = (day: string) =>
  queryOptions({
    queryKey: todoKeys.inbox(day),
    queryFn: () => unwrap(commands.todoInbox(todoClock.local())),
  });

/** The sidebar: the Inbox and Today counts and every List with its counts. */
export const listsQuery = () =>
  queryOptions({
    queryKey: todoKeys.lists(todoClock.today),
    queryFn: () => unwrap(commands.todoLists(todoClock.local())),
  });

/** All open Tasks, grouped by List (the Inbox first) unless the Preference says otherwise. */
export const allQuery = (day: string) =>
  queryOptions({
    queryKey: todoKeys.allView(day),
    queryFn: () => unwrap(commands.todoAll(todoClock.local())),
  });

/** One List: its open Tasks in groups, and its Done Tasks. */
export const listQuery = (id: string, day: string) =>
  queryOptions({
    queryKey: todoKeys.list(id, day),
    queryFn: () => unwrap(commands.todoList(id, todoClock.local())),
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

/** Days of Done fetched per page. */
const DONE_PAGE_DAYS = 14;

/** Done: whole days, newest first, a page at a time as you scroll (`before` is the last day seen). */
export const doneQuery = (day: string) =>
  infiniteQueryOptions({
    queryKey: todoKeys.done(day),
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }): Promise<DoneView> =>
      unwrap(commands.todoDone(pageParam, DONE_PAGE_DAYS, todoClock.local())),
    getNextPageParam: (last) => last.next_cursor,
  });

/** Open Tasks per date from `from` to `to`, leaving out `exclude`: the When picker's dots. */
export const dayCountsQuery = (from: string, to: string, exclude: string | null) =>
  queryOptions({
    queryKey: todoKeys.dayCounts(from, to, exclude),
    queryFn: () => unwrap(commands.todoDayCounts(from, to, exclude)),
  });

/** One Task in any state, for the Task sheet. */
export const taskQuery = (id: string) =>
  queryOptions({
    queryKey: todoKeys.task(id),
    queryFn: () => unwrap(commands.todoTask(id)),
    retry: false,
  });

/** A Task's Comments, oldest first. */
export const commentsQuery = (taskId: string) =>
  queryOptions({
    queryKey: todoKeys.comments(taskId),
    queryFn: () => unwrap(commands.todoComments(taskId)),
  });

/** A Task's History, newest line first. */
export const historyQuery = (taskId: string) =>
  queryOptions({
    queryKey: todoKeys.history(taskId),
    queryFn: () => unwrap(commands.todoTaskHistory(taskId)),
  });

/**
 * Task search results for `query`. The previous results stay on screen while the next ones load,
 * so typing never flashes empty; `enabled` is the caller's to set. Any change to a Task refreshes
 * them with the rest of Todo's queries.
 */
export const searchQuery = (query: string) =>
  queryOptions({
    queryKey: todoKeys.search(query),
    queryFn: () => unwrap(commands.todoSearch(query)),
    placeholderData: keepPreviousData,
  });

export function invalidateTodo(): Promise<void> {
  return queryClient.invalidateQueries({ queryKey: todoKeys.all });
}

/** What capture gives a new Task besides its title. */
export interface NewTask {
  title: string;
  /** The List, or none for the Inbox. */
  listId?: string | null;
  /** Due date, `YYYY-MM-DD`. */
  date?: string | null;
  /** Due time, `HH:MM`. */
  time?: string | null;
  repeat?: RepeatRule | null;
  /** 1 to 4; none or 4 is no Priority. */
  priority?: number | null;
  notes?: string;
  /** The record of understood words, as JSON. */
  understood?: string | null;
}

/**
 * Adds a Task at the bottom of a List, or of the Inbox when there is no `listId`. With a date it
 * is due that day (Today and an Upcoming day add this way); a time alone or a Repeat alone is
 * settled by Rust.
 */
export function createTask(task: NewTask): Promise<Task> {
  const repeat = task.repeat ?? null;
  const date = task.date ?? null;
  const time = task.time ?? null;
  const when =
    date === null && time === null && repeat === null
      ? null
      : { date, time, local: todoClock.local() };
  const priority = task.priority ?? null;
  const details = {
    notes: task.notes ?? '',
    priority: priority !== null && priority < 4 ? priority : null,
    repeat,
    understood: task.understood ?? null,
  };
  return unwrap(commands.todoTaskCreate(task.title, task.listId ?? null, when, details));
}

/**
 * Reads typed text for the capture box. `frozen` are stretches of an old title that must not be
 * read again (Quick edit); `ignored` and `prefer` are the user's decisions about words.
 */
export function parseCapture(
  text: string,
  words: { ignored: string[]; prefer: Record<string, string> },
  frozen: TextRange[] = [],
  at?: { today: string; now: string; weekStart: string },
): Promise<Parsed> {
  const local = todoClock.local();
  return unwrap(
    commands.todoParse({
      text,
      ignored: words.ignored,
      prefer: words.prefer,
      frozen,
      today: at?.today ?? local.today,
      now: at?.now ?? local.now,
      week_start: at?.weekStart ?? todoPrefs.weekStart,
    }),
  );
}

/** Saves a Quick edit; `quickEditUndo` puts the old values back. */
export function quickEdit(id: string, edit: QuickEdit): Promise<Task> {
  return unwrap(commands.todoTaskQuickEdit(id, edit, todoClock.local()));
}

export function quickEditUndo(id: string, edit: QuickEdit): Promise<Task> {
  return unwrap(commands.todoTaskQuickEditUndo(id, edit, todoClock.local()));
}

/**
 * Sets a Task's date, time and Repeat, or clears them. Rust refuses past dates and dates a
 * Repeat-only pick (no date) to the first matching day.
 */
export function setWhen(
  id: string,
  date: string | null,
  time: string | null,
  repeat: RepeatRule | null,
): Promise<Task> {
  return unwrap(commands.todoTaskSetWhen(id, date, time, repeat, todoClock.local()));
}

/** Moves a repeating Task to its next date without recording anything done. */
export function skipTask(id: string): Promise<Task> {
  return unwrap(commands.todoTaskSkip(id, todoClock.local()));
}

/** Puts dates and times back as they were (Undo), past dates included. */
export function restoreWhen(changes: DueChange[]): Promise<Task[]> {
  return unwrap(commands.todoTasksRestoreWhen(changes));
}

/** Moves every Overdue Task to `date`. */
export function moveOverdue(date: string): Promise<OverdueMove> {
  return unwrap(commands.todoTasksMoveOverdue(date, todoClock.local()));
}

/** Ticks a Task. A repeating one stays open on its next date and the result holds its copy. */
export function completeTask(id: string): Promise<Completion> {
  return unwrap(commands.todoTaskComplete(id, todoClock.local()));
}

export function reopenTask(id: string): Promise<Task> {
  return unwrap(commands.todoTaskReopen(id));
}

/** Puts a Task in a List, or in the Inbox when `listId` is null. */
export function moveTask(id: string, listId: string | null): Promise<Task> {
  return unwrap(commands.todoTaskMove(id, listId));
}

/** Sets a Task's Priority, 1 (highest) to 4 (none). */
export function setPriority(id: string, priority: number): Promise<Task> {
  return unwrap(commands.todoTaskSetPriority(id, priority));
}

/** Marks an open, non-repeating Task Won't do. */
export function wontDo(id: string): Promise<Task> {
  return unwrap(commands.todoTaskWontDo(id));
}

/** Deletes a Task; `restoreTask` is Undo. */
export function deleteTask(id: string): Promise<null> {
  return unwrap(commands.todoTaskDelete(id));
}

export function restoreTask(id: string): Promise<Task> {
  return unwrap(commands.todoTaskRestore(id));
}

/** Saves a Task's title and notes; saves of one `sessionId` are one History line. */
export function updateTask(
  id: string,
  title: string,
  notes: string,
  sessionId: string,
): Promise<Task> {
  return unwrap(commands.todoTaskUpdate(id, title, notes, sessionId));
}

export function addComment(taskId: string, body: string): Promise<Comment> {
  return unwrap(commands.todoCommentAdd(taskId, body));
}

export function editComment(id: string, body: string): Promise<Comment> {
  return unwrap(commands.todoCommentEdit(id, body));
}

export function deleteComment(id: string): Promise<null> {
  return unwrap(commands.todoCommentDelete(id));
}

export function restoreComment(id: string): Promise<Comment> {
  return unwrap(commands.todoCommentRestore(id));
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
  queryClient.setQueriesData<InboxView>(
    { queryKey: ['todo', 'inbox'] },
    (v) => v && withoutTaskInGroups(v, id),
  );
  queryClient.setQueriesData<AllView>(
    { queryKey: ['todo', 'all'] },
    (v) => v && withoutTaskInGroups(v, id),
  );
  queryClient.setQueriesData<ListView>(
    { queryKey: ['todo', 'list'] },
    (v) => v && withoutTaskInGroups(v, id),
  );
  queryClient.setQueriesData<TodayView>(
    { queryKey: ['todo', 'today'] },
    (v) => v && withoutTaskInGroups(v, id),
  );
  queryClient.setQueriesData<UpcomingView>(
    { queryKey: ['todo', 'upcoming'] },
    (v) => v && withoutTaskInGroups(v, id),
  );
}

/**
 * Moves a Task between two neighbours of the shared manual order, into the group `target` says
 * (what that group adopts: a day sets the date, a Priority the Priority, a List the List).
 */
export function reorderTask(
  id: string,
  before: string | null,
  after: string | null,
  target: GroupKey[],
): Promise<Task> {
  return unwrap(commands.todoTaskReorder(id, before, after, target, todoClock.local()));
}

/** Moves a List between two neighbours of the manual List order. */
export function reorderList(
  id: string,
  before: string | null,
  after: string | null,
): Promise<TodoList> {
  return unwrap(commands.todoListReorder(id, before, after));
}

/** How a List's page sorts the Tasks inside each group. */
export function setListSort(id: string, sort: TaskSort): Promise<TodoList> {
  return unwrap(commands.todoListSetSort(id, sort));
}

/** How a List's page groups its Tasks: none, priority or due. */
export function setListGroup(id: string, groupBy: GroupBy): Promise<TodoList> {
  return unwrap(commands.todoListSetGroup(id, groupBy));
}

/**
 * Saves one Preference (`todo.group_today`, `todo.inbox_sort`, `todo.lists_sort`, …) and refreshes
 * the views, which come back grouped and sorted by it.
 */
export async function savePreference(key: string, value: string): Promise<void> {
  await unwrap(commands.settingsSave([{ key, value }]));
  await queryClient.invalidateQueries({ queryKey: ['settings'] });
  await invalidateTodo();
}
