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
