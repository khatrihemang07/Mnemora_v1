import { os } from '../platform';

/** Screens that take Back for themselves (an open editor), newest last. */
const interceptors: (() => void)[] = [];

/**
 * Makes Android Back call `handler` instead of navigating, until the returned function runs. For
 * a transient mode such as inline editing, where Back means "finish".
 */
export function interceptBack(handler: () => void): () => void {
  interceptors.push(handler);
  return () => {
    const at = interceptors.lastIndexOf(handler);
    if (at !== -1) interceptors.splice(at, 1);
  };
}

/**
 * Android hardware back: finish an open editor, else go back in history, or exit at the root. A
 * no-op on desktop and in a browser, where the platform already handles back.
 */
export async function installBackButton(): Promise<void> {
  if (os !== 'android' || !('__TAURI_INTERNALS__' in window)) return;
  const { onBackButtonPress, exit } = await import('@tauri-apps/api/app');
  await onBackButtonPress(({ canGoBack }) => {
    const intercepted = interceptors.at(-1);
    if (intercepted) intercepted();
    else if (canGoBack) window.history.back();
    else void exit(0);
  });
}
