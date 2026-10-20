import { createRouter, type Routes } from 'sv-router';
import type { ModuleManifest, RoutePath } from './manifest';
import { buildRoutes } from './routes';
import { withViewTransition } from './view-transition';

/**
 * Wraps sv-router (history mode). Routes come from Module manifests, so paths are plain
 * `RoutePath` strings validated at registration instead of a generated union.
 */
export function createAppRouter(manifests: ModuleManifest[]) {
  // sv-router's generated path types can't know our runtime table, so use its string form.
  const router = createRouter(buildRoutes(manifests) as Routes) as unknown as {
    navigate(path: string): Promise<unknown>;
    p(path: string): string;
    isActive(path: string): boolean;
    route: { pathname: string };
  };
  return {
    navigate: (path: RoutePath) =>
      router.route.pathname === path
        ? Promise.resolve()
        : withViewTransition(() => router.navigate(path)),
    href: (path: RoutePath) => router.p(path),
    isActive: (path: RoutePath) => router.isActive(path),
    get pathname() {
      return router.route.pathname;
    },
  };
}
