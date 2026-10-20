import type { Component } from 'svelte';
import Monitor from '~icons/lucide/monitor';
import Moon from '~icons/lucide/moon';
import MoonStar from '~icons/lucide/moon-star';
import PanelLeft from '~icons/lucide/panel-left';
import Sun from '~icons/lucide/sun';
import { shortcutLabel } from '../lib/platform';
import { navEntries } from '../lib/router';
import type { ThemePref } from '../lib/theme/resolve';
import type { PaletteItem } from '../lib/ui/CommandPalette.svelte';
import { modules } from './registry';
import { router } from './router';

interface Actions {
  setTheme: (theme: ThemePref) => void;
  toggleSidebar: () => void;
}

const THEMES: { value: ThemePref; label: string; icon: Component<{ class?: string }> }[] = [
  { value: 'light', label: 'Switch to light theme', icon: Sun },
  { value: 'dark', label: 'Switch to dark theme', icon: Moon },
  { value: 'system', label: 'Switch to system theme', icon: Monitor },
  { value: 'oled', label: 'Switch to true black', icon: MoonStar },
];

/** Everything ⌘K can do: go to a Module, switch theme, toggle the sidebar. */
export function buildPaletteItems({ setTheme, toggleSidebar }: Actions): PaletteItem[] {
  const nav: PaletteItem[] = navEntries(modules).map((e) => ({
    id: `go-${e.id}`,
    label: `Go to ${e.manifest.name}`,
    group: 'Navigate',
    icon: e.manifest.icon,
    shortcut: e.manifest.nav.shortcut
      ? shortcutLabel(String(e.manifest.nav.shortcut))
      : e.id === 'settings'
        ? shortcutLabel(',')
        : undefined,
    run: () => void router.navigate(e.path),
  }));
  const themes: PaletteItem[] = THEMES.map((t) => ({
    id: `theme-${t.value}`,
    label: t.label,
    group: 'Appearance',
    icon: t.icon,
    run: () => setTheme(t.value),
  }));
  return [
    ...nav,
    ...themes,
    {
      id: 'toggle-sidebar',
      label: 'Toggle sidebar',
      group: 'View',
      icon: PanelLeft,
      shortcut: shortcutLabel('\\'),
      run: toggleSidebar,
    },
  ];
}
