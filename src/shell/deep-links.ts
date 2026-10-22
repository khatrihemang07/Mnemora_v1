import { commands, events } from '../lib/ipc/bindings';
import type { ModuleManifest, RoutePath } from '../lib/router';
import { modules } from './registry';
import { router } from './router';

/**
 * Where a link `mnemora://‹module›?...` goes. The host names the Module; its `openLink` hook sets
 * what the link names and returns the path. An unknown host, a Module without the hook or a link
 * that does not parse goes Home.
 */
export function routeForLink(raw: string, manifests: readonly ModuleManifest[]): RoutePath {
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return '/';
  }
  if (url.protocol !== 'mnemora:') return '/';
  return manifests.find((m) => m.id === url.hostname)?.openLink?.(url) ?? '/';
}

interface Deps {
  take: () => Promise<string[]>;
  onOpened: (handler: () => void) => Promise<() => void>;
  navigate: (path: RoutePath) => void;
  manifests: readonly ModuleManifest[];
}

/**
 * Follows the links that open the app: the ones queued before this ran (a cold start from the Todo
 * widget) and, while the app runs, each one Rust announces. Only the newest of several matters.
 * Returns the unlisten function.
 */
export async function startLinkBridge(deps: Deps): Promise<() => void> {
  const drain = async () => {
    const links = await deps.take();
    const newest = links.at(-1);
    if (newest !== undefined) deps.navigate(routeForLink(newest, deps.manifests));
  };
  // Listening first, so a link that lands during the first drain is not missed.
  const unlisten = await deps.onOpened(() => void drain());
  await drain();
  return unlisten;
}

/** The app's own wiring of `startLinkBridge`. */
export function startAppLinkBridge(): Promise<() => void> {
  return startLinkBridge({
    take: () => commands.appTakeLinks(),
    onOpened: (handler) => events.linkOpenedEvent.listen(handler),
    navigate: (path) => void router.navigate(path),
    manifests: modules,
  });
}
