import type { Component } from 'svelte';

export type RoutePath = `/${string}`;
export type RouteLoader = () => Promise<{ default: Component }>;

/** What the shell hands to a Module's actions, so Modules never import the shell. */
export interface ModuleContext {
  navigate: (path: RoutePath) => void;
}

/** A command a Module adds to the ⌘K palette. */
export interface ModuleAction {
  id: string;
  label: string;
  icon?: Component<{ class?: string }>;
  shortcut?: string;
  run: (ctx: ModuleContext) => void;
}

/** What a Module tells the shell about itself. Registered in one place: `shell/registry.ts`. */
export interface ModuleManifest {
  id: string;
  name: string;
  icon: Component<{ class?: string }>;
  /** CSS custom property holding this Module's hue, e.g. `--hue-home`. */
  accentVar: string;
  routes: { path: RoutePath; component: RouteLoader }[];
  /** `shortcut` is the digit used with the modifier key (Cmd/Ctrl+1..9), or null. */
  nav: { order: number; shortcut: number | null; placement?: 'main' | 'bottom' };
  /** Home and Settings: always on, no switch in Settings ▸ Modules. */
  required?: true;
  /** A card on Home, loaded only when Home renders it. */
  homeCard?: RouteLoader;
  /** The Module's own Settings section, appended after the core sections. */
  settingsSection?: { label: string; icon: Component<{ class?: string }>; component: RouteLoader };
  /** What ⌘N does while this Module is enabled (the first enabled Module with one wins). */
  create?: (ctx: ModuleContext) => void;
  paletteActions?: ModuleAction[];
}
