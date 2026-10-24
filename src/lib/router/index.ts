export { blockNavigation } from 'sv-router';
export { closeOnBack, installBackButton, interceptBack } from './back-button.svelte';
export { createAppRouter } from './create';
export { dynamicActions } from './dynamic-actions.svelte';
export type {
  BareKey,
  DeviceSwitch,
  HomeCardLoader,
  HomeCardProps,
  ModuleAction,
  ModuleBadge,
  ModuleContext,
  ModuleManifest,
  NavFlyoutProps,
  NavTree,
  NavTreeProps,
  PhoneTabActions,
  RouteLoader,
  RoutePath,
} from './manifest';
export { default as RouterOutlet } from './RouterOutlet.svelte';
export type { NavEntry } from './routes';
export { buildRoutes, navEntries } from './routes';
