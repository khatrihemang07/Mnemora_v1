import type { Component } from 'svelte';

export type RoutePath = `/${string}`;
export type RouteLoader = () => Promise<{ default: Component }>;

/** What a Home card receives: the shell's context, so its buttons can navigate. */
export interface HomeCardProps {
  ctx: ModuleContext;
}
export type HomeCardLoader = () => Promise<{ default: Component<HomeCardProps> }>;

/** A count beside a Module's name in the sidebar and the phone tab bar; red when `alert`. */
export interface ModuleBadge {
  count: number;
  alert: boolean;
}

/** What the shell hands to a Module's actions, so Modules never import the shell. */
export interface ModuleContext {
  navigate: (path: RoutePath) => void;
  /** Whether the app is on that path (a sidebar tree shows its active row only inside its Module). */
  isActive: (path: RoutePath) => boolean;
}

/**
 * A bare single-key shortcut a Module owns across the whole app (ADR 0011): Q for Todo's quick
 * add. It fires only when nothing is being typed into and no dialog or menu is open, with no
 * modifier held. The first enabled Module (by nav order) that claims a key gets it.
 */
export interface BareKey {
  /** A single letter, either case. */
  key: string;
  run: (ctx: ModuleContext) => void;
}

/**
 * What a Module does with its own tab in the phone tab bar, beyond going to its page: when the tab
 * is tapped while the Module is already on screen, and when it is long-pressed (from any page).
 */
export interface PhoneTabActions {
  onActiveTap?: (ctx: ModuleContext) => void;
  onLongPress?: (ctx: ModuleContext) => void;
}

/** What the shell gives a Module's sidebar tree. */
export interface NavTreeProps {
  ctx: ModuleContext;
}
/** What the shell gives a Module's rail flyout; `onclose` dismisses it after a pick. */
export interface NavFlyoutProps {
  ctx: ModuleContext;
  onclose: () => void;
}

/**
 * A Module's own navigation nested under its sidebar row (Todo's views and Lists). The row gets a
 * chevron and the tree unfolds by the fold rule in `shell/tree-fold.ts`; on the icon rail the
 * flyout shows the same destinations on hover or right-click.
 */
export interface NavTree {
  tree: () => Promise<{ default: Component<NavTreeProps> }>;
  flyout: () => Promise<{ default: Component<NavFlyoutProps> }>;
}

/** A command a Module adds to the ⌘K palette. */
export interface ModuleAction {
  id: string;
  label: string;
  icon?: Component<{ class?: string }>;
  shortcut?: string;
  /**
   * Makes it a query action: it appears once something is typed in the palette, labelled with
   * that text ("Search Journal for 'river'"), and `run` receives it.
   */
  queryLabel?: (query: string) => string;
  /**
   * For a query action: a short line saying what `run` would do with the text (Todo's "Add task"
   * shows how it read the date, Priority and List). Asked as the text changes; none shows nothing.
   */
  queryPreview?: (query: string) => Promise<string | null>;
  run: (ctx: ModuleContext, query: string) => void;
}

/**
 * A Module's on/off switch when it is a Device setting rather than a Preference (Sync): read from
 * the device file and never synced, so each device chooses for itself. Created during component
 * setup, like `useBadge`, so it may hold queries.
 */
export interface DeviceSwitch {
  /** Whether the Module is on for this device. */
  readonly on: boolean;
  /** The device file has been read, so `on` can be trusted (until then it reads off). */
  readonly ready: boolean;
  /** Asks for it on or off. Turning off may ask first; turning on may open the Module. */
  set: (on: boolean) => void;
}

/** What a Module tells the shell about itself. Registered in one place: `shell/registry.ts`. */
export interface ModuleManifest {
  id: string;
  name: string;
  /** One line under the name in Settings ▸ Modules, for a Module that needs explaining. */
  description?: string;
  icon: Component<{ class?: string }>;
  /** CSS custom property holding this Module's hue, e.g. `--hue-home`. */
  accentVar: string;
  routes: { path: RoutePath; component: RouteLoader }[];
  /** `shortcut` is the digit used with the modifier key (Cmd/Ctrl+1..9), or null. */
  nav: { order: number; shortcut: number | null; placement?: 'main' | 'bottom' };
  /** Nested navigation under the Module's sidebar row. */
  navTree?: NavTree;
  /** Home and Settings: always on, no switch in Settings ▸ Modules. */
  required?: true;
  /**
   * `false`: off until the user opts in (Sync). Enabled = (default on and not disabled) or
   * (default off and opted in); see `enabledModules`. Absent means on by default.
   */
  defaultEnabled?: false;
  /** For a default-off Module whose on/off is a Device setting: the shell reads and sets it here. */
  deviceSwitch?: (ctx: ModuleContext) => DeviceSwitch;
  /**
   * The Module's badge. Called while a sidebar row or tab renders (it may create queries), and
   * read reactively; none means nothing to show.
   */
  useBadge?: () => { readonly badge: ModuleBadge | null };
  /** A card on Home, loaded only when Home renders it. */
  homeCard?: HomeCardLoader;
  /** The Module's own Settings section, appended after the core sections. */
  settingsSection?: { label: string; icon: Component<{ class?: string }>; component: RouteLoader };
  /** What ⌘N does while this Module is enabled (the first enabled Module with one wins). */
  create?: (ctx: ModuleContext) => void;
  paletteActions?: ModuleAction[];
  /** What its phone tab does when tapped again or long-pressed (Todo: back to Today, Quick add). */
  phoneTab?: PhoneTabActions;
  /** Bare single-key shortcuts that work anywhere in the app. */
  bareKeys?: BareKey[];
  /** A component the shell mounts for as long as the Module is on (a floating box a key opens). */
  overlay?: () => Promise<{ default: Component }>;
  /**
   * A link that opens the app on this Module (`mnemora://‹id›?...`, from the Todo widget). It sets
   * whatever the link names and returns the path to go to, with a query if the page takes one.
   * Without it the link goes Home.
   */
  openLink?: (url: URL) => RoutePath;
}
