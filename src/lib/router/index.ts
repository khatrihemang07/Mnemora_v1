export { blockNavigation } from 'sv-router';
export { installBackButton, interceptBack } from './back-button';
export { createAppRouter } from './create';
export type {
  HomeCardLoader,
  HomeCardProps,
  ModuleAction,
  ModuleContext,
  ModuleManifest,
  RouteLoader,
  RoutePath,
} from './manifest';
export { default as RouterOutlet } from './RouterOutlet.svelte';
export type { NavEntry } from './routes';
export { buildRoutes, navEntries } from './routes';
