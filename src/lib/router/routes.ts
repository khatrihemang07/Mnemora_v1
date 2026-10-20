import type { ModuleManifest, RouteLoader, RoutePath } from './manifest';

/** Flattens Module manifests into the route table; two Modules can't claim one path. */
export function buildRoutes(manifests: ModuleManifest[]): Record<RoutePath, RouteLoader> {
  const routes: Record<string, RouteLoader> = {};
  for (const m of manifests) {
    for (const r of m.routes) {
      if (r.path in routes) throw new Error(`Route ${r.path} is registered twice (${m.id})`);
      routes[r.path] = r.component;
    }
  }
  return routes;
}

export interface NavEntry {
  id: string;
  path: RoutePath;
  manifest: ModuleManifest;
}

/** Nav entries in display order, each linking to the Module's first route. */
export function navEntries(manifests: ModuleManifest[]): NavEntry[] {
  return [...manifests]
    .sort((a, b) => a.nav.order - b.nav.order)
    .flatMap((m) => {
      const first = m.routes[0];
      return first ? [{ id: m.id, path: first.path, manifest: m }] : [];
    });
}
