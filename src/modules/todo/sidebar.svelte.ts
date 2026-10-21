import { createQuery } from '@tanstack/svelte-query';
import type { SidebarView, TodoList } from '../../lib/ipc/bindings';
import { listsQuery } from './queries';

/**
 * The sidebar's data for a page that shows Tasks from several Lists: the Lists by id (a row's List
 * chip, a group's heading) and the raw view (the Lists' sort). Call once while the page is made.
 */
export function useSidebar() {
  const query = createQuery(() => listsQuery());
  const lists = $derived<ReadonlyMap<string, TodoList>>(
    new Map((query.data?.lists ?? []).map((e) => [e.list.id, e.list])),
  );
  return {
    get lists() {
      return lists;
    },
    get view(): SidebarView | undefined {
      return query.data;
    },
  };
}
