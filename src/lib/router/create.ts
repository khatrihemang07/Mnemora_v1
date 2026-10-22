import { createRouter, type Routes } from 'sv-router';
import type { ModuleManifest, RoutePath } from './manifest';
import { splitPath } from './path';
import { buildRoutes } from './routes';
import { withViewTransition } from './view-transition';

/**
 * Wraps sv-router (history mode). Routes come from Module manifests, so paths are plain
 * `RoutePath` strings validated at registration instead of a generated union.
 */
export function createAppRouter(manifests: ModuleManifest[]) {
  // sv-router's generated path types can't know our runtime table, so use its string form.
  const router = createRouter(buildRoutes(manifests) as Routes) as unknown as {
    navigate(path: string, options?: { search?: string; replace?: boolean }): Promise<unknown>;
    p(path: string): string;
    isActive(path: string): boolean;
    route: { pathname: string };
  };
  return {
    // A path may carry a query (`/todo?task=‹id›`). Landing on the page already open with a query
    // swaps the history entry instead of adding one, so Back does not walk through a no-op.
    navigate: (path: RoutePath, options?: { replace?: boolean }) => {
      const { pathname, search } = splitPath(path);
      const here = router.route.pathname === pathname;
      if (here && !search) return Promise.resolve();
      const replace = here || options?.replace === true;
      return withViewTransition(() =>
        router.navigate(pathname, search || replace ? { search, replace } : undefined),
      );
    },
    href: (path: RoutePath) => router.p(path),
    isActive: (path: RoutePath) => router.isActive(path),
    get pathname() {
      return router.route.pathname;
    },
  };
}
