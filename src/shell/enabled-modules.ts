import type { BareKey, HomeCardLoader, ModuleManifest, RoutePath } from '../lib/router';
import { activeModuleId } from './phone-tabs';

const byOrder = (a: ModuleManifest, b: ModuleManifest) => a.nav.order - b.nav.order;

/**
 * A Module is on when it is on by default and not listed as disabled, or off by default and listed
 * as opted in (`optedIn` holds the Preference's list plus any Module whose switch is a Device
 * setting and on); required Modules ignore both lists.
 */
export function enabledModules(
  manifests: ModuleManifest[],
  disabled: string[],
  optedIn: string[] = [],
): ModuleManifest[] {
  return manifests.filter(
    (m) =>
      m.required ||
      (m.defaultEnabled === false ? optedIn.includes(m.id) : !disabled.includes(m.id)),
  );
}

/** The new opted-in list after switching a default-off Module on or off. */
export function setOptedIn(optedIn: string[], id: string, on: boolean): string[] {
  const rest = optedIn.filter((d) => d !== id);
  return on ? [...rest, id] : rest;
}

/** The new disabled list after switching one Module on or off. */
export function setModuleEnabled(disabled: string[], id: string, on: boolean): string[] {
  const rest = disabled.filter((d) => d !== id);
  return on ? rest : [...rest, id];
}

/** What ⌘N runs: the first enabled Module (by nav order) with a create action. */
export const createTarget = (enabled: ModuleManifest[]) =>
  [...enabled].sort(byOrder).find((m) => m.create);

/** The bare key `key` (a letter, either case) is owned by the first enabled Module that claims it. */
export function bareKeyFor(enabled: ModuleManifest[], key: string): BareKey | null {
  const wanted = key.toLowerCase();
  for (const m of [...enabled].sort(byOrder)) {
    const hit = m.bareKeys?.find((k) => k.key.toLowerCase() === wanted);
    if (hit) return hit;
  }
  return null;
}

/** The enabled Modules that mount an always-on overlay, in nav order. */
export const overlayModules = (enabled: ModuleManifest[]) =>
  [...enabled].sort(byOrder).filter((m) => m.overlay);

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
