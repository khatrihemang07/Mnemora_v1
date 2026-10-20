export { blockNavigation } from 'sv-router';
export { installBackButton } from './back-button';
export { createAppRouter } from './create';
export type {
  ModuleAction,
  ModuleContext,
  ModuleManifest,
  RouteLoader,
  RoutePath,
} from './manifest';
export { default as RouterOutlet } from './RouterOutlet.svelte';
export type { NavEntry } from './routes';
export { buildRoutes, navEntries } from './routes';
