import { commands } from '../lib/ipc/bindings';
import { unwrap } from '../lib/ipc/unwrap';
import type { ModuleManifest } from '../lib/router';
import { notifyError } from '../lib/ui/toast';
import { viewport } from '../lib/viewport.svelte';
import { buildPaletteItems } from './palette-items';
import { shell } from './shell-state.svelte';
import type { ShortcutAction } from './shortcuts';

/** Palette theme commands are explicit, so they apply and save at once (no draft). */
async function setTheme(theme: string) {
  try {
    await unwrap(commands.settingsSave([{ key: 'core.theme', value: theme }]));
  } catch (e) {
    notifyError(e instanceof Error ? e.message : 'Could not change theme');
  }
}

/** ⌘K items for the given enabled Modules. */
export const paletteItemsFor = (enabled: ModuleManifest[]) =>
  buildPaletteItems({
    setTheme: (t) => void setTheme(t),
    toggleSidebar: () => shell.toggleSidebar(),
    enabled,
  });

/** Carries out a resolved keyboard shortcut. */
export function runShortcut(a: ShortcutAction) {
  switch (a.type) {
    case 'palette':
      shell.paletteOpen = !shell.paletteOpen;
      break;
    case 'sidebar':
      if (!viewport.isPhone) shell.toggleSidebar();
      break;
  }
}
