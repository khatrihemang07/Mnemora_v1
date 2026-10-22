import type { RoutePath } from './manifest';

/** A route path that may carry a query (`/todo?task=‹id›`), split for the router. */
export function splitPath(path: string): { pathname: RoutePath; search: string } {
  const at = path.indexOf('?');
  const pathname = (at === -1 ? path : path.slice(0, at)) as RoutePath;
  return { pathname, search: at === -1 ? '' : path.slice(at + 1) };
}
