import type { HomeCardLoader, ModuleManifest, RoutePath } from '../lib/router';
import { activeModuleId } from './phone-tabs';

const byOrder = (a: ModuleManifest, b: ModuleManifest) => a.nav.order - b.nav.order;

/** A Module is on unless it is listed as disabled; required Modules ignore the list. */
export function enabledModules(manifests: ModuleManifest[], disabled: string[]): ModuleManifest[] {
  return manifests.filter((m) => m.required || !disabled.includes(m.id));
}

/** The new disabled list after switching one Module on or off. */
export function setModuleEnabled(disabled: string[], id: string, on: boolean): string[] {
  const rest = disabled.filter((d) => d !== id);
  return on ? rest : [...rest, id];
}

/** What ⌘N runs: the first enabled Module (by nav order) with a create action. */
export const createTarget = (enabled: ModuleManifest[]) =>
  [...enabled].sort(byOrder).find((m) => m.create);

/** Home cards of the enabled Modules, in nav order. */
export function homeCards(enabled: ModuleManifest[]): { id: string; load: HomeCardLoader }[] {
  return [...enabled]
    .sort(byOrder)
    .flatMap((m) => (m.homeCard ? [{ id: m.id, load: m.homeCard }] : []));
}

/** Settings sections the enabled Modules add after the core ones. */
export function moduleSettingsSections(enabled: ModuleManifest[]) {
  return [...enabled]
    .sort(byOrder)
    .flatMap((m) => (m.settingsSection ? [{ id: m.id, ...m.settingsSection }] : []));
}

/** Where to go instead when the current route belongs to a disabled Module; null if it's fine. */
export function redirectForDisabled(
  pathname: string,
  all: ModuleManifest[],
  enabled: ModuleManifest[],
): RoutePath | null {
  const entries = all.flatMap((m) => m.routes.map((r) => ({ id: m.id, path: r.path })));
  const owner = activeModuleId(pathname, entries);
  return owner && !enabled.some((m) => m.id === owner) ? '/' : null;
}
