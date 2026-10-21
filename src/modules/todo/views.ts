/** Which page of Todo is showing. Todo holds it as state on its single `/todo` route. */
export type SmartViewId = 'inbox' | 'today' | 'upcoming' | 'all' | 'done';
export type TodoView = { kind: 'smart'; id: SmartViewId } | { kind: 'list'; id: string };

/** The views under "Todo" in the sidebar, in order. */
export const SMART_VIEWS: readonly { id: SmartViewId; label: string }[] = [
  { id: 'inbox', label: 'Inbox' },
  { id: 'today', label: 'Today' },
  { id: 'upcoming', label: 'Upcoming' },
  { id: 'all', label: 'All' },
  { id: 'done', label: 'Done' },
];

/** The count beside a view in the sidebar: Inbox and Today have one, and Today's is red while anything is Overdue. */
export function viewCount(
  sidebar: { inbox_count: number; today_count: number; overdue_count: number } | undefined,
  id: SmartViewId,
): { count: number; alert: boolean } {
  if (!sidebar) return { count: 0, alert: false };
  if (id === 'inbox') return { count: sidebar.inbox_count, alert: false };
  if (id === 'today') return { count: sidebar.today_count, alert: sidebar.overdue_count > 0 };
  return { count: 0, alert: false };
}

/** Todo opens here at launch (L4); later it reopens on the last view used. */
export const DEFAULT_VIEW: TodoView = { kind: 'smart', id: 'today' };

export const smartView = (id: SmartViewId): TodoView => ({ kind: 'smart', id });
export const listView = (id: string): TodoView => ({ kind: 'list', id });

export const sameView = (a: TodoView, b: TodoView): boolean => a.kind === b.kind && a.id === b.id;

export const viewLabel = (id: SmartViewId): string =>
  SMART_VIEWS.find((v) => v.id === id)?.label ?? id;

/** Where to land when a List goes away while it is open: the Inbox, where moved Tasks arrive. */
export function viewAfterListGone(view: TodoView, listId: string): TodoView {
  return view.kind === 'list' && view.id === listId ? smartView('inbox') : view;
}

/** The phone's pinned segments, left to right; ☰ at the end holds the rest. */
export const PHONE_SEGMENTS: readonly SmartViewId[] = ['today', 'upcoming', 'inbox'];

/** Which segment is lit: one of the three, or `more` (☰) for All, Done and the Lists. */
export function activeSegment(view: TodoView): SmartViewId | 'more' {
  return view.kind === 'smart' && PHONE_SEGMENTS.includes(view.id) ? view.id : 'more';
}

/**
 * The view a swipe on the title or segment bar moves to: the next (`1`) or previous (`-1`) of the
 * three segments. It stops at the ends, and does nothing from a view that is not a segment.
 */
export function swipedView(view: TodoView, step: -1 | 1): TodoView | null {
  if (view.kind !== 'smart') return null;
  const at = PHONE_SEGMENTS.indexOf(view.id);
  const to = PHONE_SEGMENTS[at + step];
  return at === -1 || to === undefined ? null : smartView(to);
}

/**
 * Where the phone's + adds: the Task is due today in Today, tomorrow in Upcoming, in the List in a
 * List, and in the Inbox elsewhere.
 */
export function addPlaceFor(
  view: TodoView,
  today: string,
  tomorrow: string,
): { date: string | null; listId: string | null } {
  if (view.kind === 'list') return { date: null, listId: view.id };
  if (view.id === 'today') return { date: today, listId: null };
  if (view.id === 'upcoming') return { date: tomorrow, listId: null };
  return { date: null, listId: null };
}

/**
 * Whether a new open Task shows on `view` as it stands: Today holds what is due today or earlier,
 * Upcoming what is due later, the Inbox what has no List (and undated ones from Lists), a List its
 * own Tasks, All everything open, and Done nothing open.
 */
export function visibleOn(
  view: TodoView,
  task: { due_date: string | null; list_id: string | null },
  today: string,
): boolean {
  if (view.kind === 'list') return task.list_id === view.id;
  switch (view.id) {
    case 'today':
      return task.due_date !== null && task.due_date <= today;
    case 'upcoming':
      return task.due_date !== null && task.due_date > today;
    case 'inbox':
      return task.list_id === null || task.due_date === null;
    case 'all':
      return true;
    case 'done':
      return false;
  }
}
