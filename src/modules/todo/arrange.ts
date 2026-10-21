import type {
  DoneInLists,
  DueBucket,
  GroupBy,
  GroupKey,
  ListSort,
  Task,
  TaskGroup,
  TaskSort,
  TodoList,
} from '../../lib/ipc/bindings';
import { dayHeading, nextText } from './dates';
import { PRIORITY_NAMES } from './tasks';

/** A choice in a Group by or Sort menu. */
export interface Option<T extends string> {
  value: T;
  label: string;
}

/** The views whose grouping is a Preference. A List keeps its own on its row. */
export type PrefView = 'today' | 'upcoming' | 'inbox' | 'all';

/** The Group by choices of each view, as they read in the menu. Rust accepts exactly these. */
export const GROUP_OPTIONS: Record<PrefView | 'list', readonly Option<GroupBy>[]> = {
  today: [
    { value: 'none', label: 'None' },
    { value: 'priority', label: 'Priority' },
    { value: 'list', label: 'List' },
  ],
  upcoming: [
    { value: 'day', label: 'Day' },
    { value: 'priority', label: 'Priority' },
    { value: 'list', label: 'List' },
  ],
  inbox: [
    { value: 'none', label: 'None' },
    { value: 'priority', label: 'Priority' },
    { value: 'due', label: 'Due date' },
  ],
  all: [
    { value: 'list', label: 'List' },
    { value: 'priority', label: 'Priority' },
    { value: 'due', label: 'Due date' },
    { value: 'none', label: 'None' },
  ],
  list: [
    { value: 'none', label: 'None' },
    { value: 'priority', label: 'Priority' },
    { value: 'due', label: 'Due date' },
  ],
};

/** What each view groups by until the user chooses. */
export const GROUP_DEFAULTS: Record<PrefView | 'list', GroupBy> = {
  today: 'priority',
  upcoming: 'day',
  inbox: 'none',
  all: 'list',
  list: 'none',
};

/** The Preference that holds each view's grouping. */
export const GROUP_PREF_KEYS: Record<PrefView, string> = {
  today: 'todo.group_today',
  upcoming: 'todo.group_upcoming',
  inbox: 'todo.group_inbox',
  all: 'todo.group_all',
};

/** The Preference that holds the Inbox's sort. */
export const INBOX_SORT_PREF = 'todo.inbox_sort';
/** The Preference that holds how the Lists in the sidebar are sorted. */
export const LISTS_SORT_PREF = 'todo.lists_sort';
/** The Preference that holds what Done Tasks do on a List page (its Settings screen is separate). */
export const DONE_IN_LISTS_PREF = 'todo.done_in_lists';

/** How a List's (or the Inbox's) Tasks can be sorted inside each group. */
export const TASK_SORT_OPTIONS: readonly Option<TaskSort>[] = [
  { value: 'manual', label: 'Manual' },
  { value: 'due', label: 'Due date' },
  { value: 'priority', label: 'Priority' },
  { value: 'title', label: 'Title' },
  { value: 'added', label: 'Recently added' },
];

/** How the Lists in the sidebar can be sorted. */
export const LIST_SORT_OPTIONS: readonly Option<ListSort>[] = [
  { value: 'manual', label: 'Manual' },
  { value: 'name_asc', label: 'Name A–Z' },
  { value: 'name_desc', label: 'Name Z–A' },
  { value: 'most_open', label: 'Most open' },
  { value: 'created', label: 'Oldest first' },
];

/** What Done Tasks in Lists can do: the Settings screen offers these. */
export const DONE_IN_LISTS_OPTIONS: readonly Option<DoneInLists>[] = [
  { value: 'today_shown', label: 'Today shown, older folded' },
  { value: 'folded', label: 'All folded' },
  { value: 'shown', label: 'All shown' },
  { value: 'hidden', label: 'Hidden' },
];

/** The label of the chosen option; the value itself when the table does not know it. */
export const optionLabel = <T extends string>(options: readonly Option<T>[], value: T): string =>
  options.find((o) => o.value === value)?.label ?? value;

/** The Group button: `Group: Priority`. */
export const groupButtonLabel = (view: PrefView | 'list', value: GroupBy): string =>
  `Group: ${optionLabel(GROUP_OPTIONS[view], value)}`;

/** A sort as the menu names it: Manual says it is the drag order. */
export const sortMenuLabel = (option: Option<string>): string =>
  option.value === 'manual' ? 'Manual (drag)' : option.label;

/** Whether a sort lets the user drag: only the manual order does. */
export const canDrag = (sort: TaskSort): boolean => sort === 'manual';

/** How a group's heading looks. */
export type GroupHeading =
  | { kind: 'none' }
  | {
      kind: 'label';
      label: string;
      tone: 'plain' | 'danger';
      /** 1 to 3 for a Priority group: the heading wears the flag of that Priority. */
      priority?: number;
      icon?: 'calendar-off';
    }
  | { kind: 'day'; name: string; small: string }
  | { kind: 'list'; list: TodoList | null };

const DUE_LABELS: Record<DueBucket, string> = {
  overdue: 'Overdue',
  today: 'Today',
  tomorrow: 'Tomorrow',
  week: 'Next 7 days',
  later: 'Later',
  no_date: 'No date',
};

/**
 * What heads a group. `showToday` is whether today's own group gets a "Today" heading (Today does
 * so only under an Overdue group); a plain group never has one.
 */
export function groupHeading(
  key: GroupKey,
  ctx: { today: string; lists: ReadonlyMap<string, TodoList>; showToday: boolean },
): GroupHeading {
  switch (key.kind) {
    case 'plain':
      return { kind: 'none' };
    case 'overdue':
      return { kind: 'label', label: 'Overdue', tone: 'danger' };
    case 'priority':
      return {
        kind: 'label',
        label: key.priority === 4 ? 'No priority' : `P${key.priority}`,
        tone: 'plain',
        priority: key.priority < 4 ? key.priority : undefined,
      };
    case 'list':
      return {
        kind: 'list',
        list: key.list_id === null ? null : (ctx.lists.get(key.list_id) ?? null),
      };
    case 'day':
      if (key.date === ctx.today) {
        return ctx.showToday ? { kind: 'label', label: 'Today', tone: 'plain' } : { kind: 'none' };
      }
      return { kind: 'day', ...dayHeading(key.date, ctx.today) };
    case 'due':
      return {
        kind: 'label',
        label: DUE_LABELS[key.bucket],
        tone: key.bucket === 'overdue' ? 'danger' : 'plain',
      };
    case 'no_date_in_list':
      return { kind: 'label', label: 'No date, in a List', tone: 'plain', icon: 'calendar-off' };
  }
}

/** What a Task added inside a group starts with: its day and its List (none leaves them to the page). */
export function addDefaults(adopt: readonly GroupKey[]): {
  date: string | null;
  listId?: string | null;
} {
  let date: string | null = null;
  let listId: string | null | undefined;
  for (const key of adopt) {
    if (key.kind === 'day') date = key.date;
    // Priorities are never inherited: a Task added under P1 is P4 unless it says otherwise.
    if (key.kind === 'list') listId = key.list_id;
  }
  return listId === undefined ? { date } : { date, listId };
}

/**
 * Whether a group takes new Tasks ("+ New task" at its end). Overdue and the Inbox's "No date, in a
 * List" are for what already exists.
 */
export const groupTakesTasks = (group: TaskGroup): boolean =>
  group.key.kind !== 'overdue' &&
  group.key.kind !== 'no_date_in_list' &&
  !(group.key.kind === 'due' && group.key.bucket === 'overdue');

/**
 * What dropping a Task into a group changes about it, in words for the drag ghost and the Undo toast
 * (`Fri 24 Oct`, `P1`, `🏠 Home`); none when the Task already has everything the group adopts.
 */
export function adoptSummary(
  adopt: readonly GroupKey[],
  task: Pick<Task, 'due_date' | 'priority' | 'list_id'>,
  ctx: { today: string; lists: ReadonlyMap<string, TodoList> },
): string | null {
  const parts: string[] = [];
  for (const key of adopt) {
    if (key.kind === 'day' && task.due_date !== key.date) {
      parts.push(key.date === ctx.today ? 'Today' : nextText(key.date, ctx.today));
    } else if (key.kind === 'priority' && task.priority !== key.priority) {
      parts.push(
        key.priority === 4 ? 'No priority' : (PRIORITY_NAMES[key.priority] ?? `P${key.priority}`),
      );
    } else if (key.kind === 'list' && task.list_id !== key.list_id) {
      const list = key.list_id === null ? null : ctx.lists.get(key.list_id);
      parts.push(list ? `${list.emoji ?? '≡'} ${list.name}` : '📥 Inbox');
    }
  }
  return parts.length > 0 ? parts.join(' · ') : null;
}
