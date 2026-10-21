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
