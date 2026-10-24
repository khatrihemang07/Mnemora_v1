import type { RoutePath } from './manifest';

/** A route path that may carry a query (`/todo?task=‹id›`), split for the router. */
export function splitPath(path: string): { pathname: RoutePath; search: string } {
  const at = path.indexOf('?');
  const pathname = (at === -1 ? path : path.slice(0, at)) as RoutePath;
  return { pathname, search: at === -1 ? '' : path.slice(at + 1) };
}

/** Whether `pathname` is `path` or a page below it (`/sync/activity` is within `/sync`). */
export const isWithin = (pathname: string, path: string): boolean =>
  pathname === path || (path !== '/' && pathname.startsWith(`${path}/`));
