import type { RoutePath } from '../../lib/router';
import { urlParam } from '../../lib/router/url-param';
import { todoNav } from './nav.svelte';
import { listView, smartView, type TodoView } from './views';

/** What a Todo link names: a view as the Todo widget spells it, and a Task, either may be none. */
export interface TodoLink {
  /** `today`, `upcoming`, `inbox`, `all` or `list:‹id›`; none when the link names no view we know. */
  view: string | null;
  task: string | null;
}

const FIXED = ['today', 'upcoming', 'inbox', 'all'] as const;

/** Reads `mnemora://todo?view=...&task=...`. Anything unknown is left out, never an error. */
export function parseTodoLink(url: URL): TodoLink {
  const raw = url.searchParams.get('view');
  const listId = raw?.startsWith('list:') ? raw.slice('list:'.length) : '';
  const known = raw !== null && ((FIXED as readonly string[]).includes(raw) || listId !== '');
  return { view: known ? raw : null, task: url.searchParams.get('task') || null };
}

/** The view a link names, as Todo holds it. */
export function viewOfLink(view: string): TodoView {
  return view.startsWith('list:')
    ? listView(view.slice('list:'.length))
    : smartView(view as (typeof FIXED)[number]);
}

/**
 * Todo's end of a link: shows the view the link names and says where to go. A Task opens in its
 * sheet over that view (`/todo?task=‹id›`); a List that no longer exists falls back inside Todo, the
 * way it does when a List is deleted elsewhere.
 */
export function openTodoLink(url: URL): RoutePath {
  const { view, task } = parseTodoLink(url);
  if (view) todoNav.go(viewOfLink(view));
  if (task) return `/todo?task=${encodeURIComponent(task)}`;
  // A header tap while a Task is open shows the view, not the sheet.
  if (urlParam.get('task') !== null) urlParam.delete('task', true);
  return '/todo';
}
