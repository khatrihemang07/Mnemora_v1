import type { Component } from 'svelte';

export type RoutePath = `/${string}`;
export type RouteLoader = () => Promise<{ default: Component }>;

/** What a Module tells the shell about itself. Registered in one place: `shell/registry.ts`. */
export interface ModuleManifest {
  id: string;
  name: string;
  icon: Component<{ class?: string }>;
  /** CSS custom property holding this Module's hue, e.g. `--color-home`. */
  accentVar: string;
  routes: { path: RoutePath; component: RouteLoader }[];
  /** `shortcut` is the digit used with the modifier key (Cmd/Ctrl+1..9), or null. */
  nav: { order: number; shortcut: number | null; placement?: 'main' | 'bottom' };
  settingsSection?: string;
}
