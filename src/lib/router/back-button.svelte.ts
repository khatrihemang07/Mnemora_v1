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
 * For an overlay (Sheet, Popover, Menu, Dialog): while `isOpen()` is true, Android Back calls
 * `close` instead of navigating. Call it during component init; it registers its own effect.
 */
export function closeOnBack(isOpen: () => boolean, close: () => void): void {
  $effect(() => {
    if (!isOpen()) return;
    return interceptBack(close);
  });
}

interface BackDeps {
  goBack: () => void;
  /** Leaves the app at the root. */
  leave: () => void;
}

/**
 * One Back press: an open editor or overlay first, then history, then leave the app. Leaving
 * moves the task to the back like any Android app; `exit(0)` would run libc exit destructors
 * while the render thread still draws, which aborts (#51). It stays only as a fallback.
 */
export function handleBack({ canGoBack }: { canGoBack: boolean }, deps: BackDeps): void {
  const intercepted = interceptors.at(-1);
  if (intercepted) intercepted();
  else if (canGoBack) deps.goBack();
  else deps.leave();
}

/**
 * Android hardware back: finish an open editor, else go back in history, or leave at the root. A
 * no-op on desktop and in a browser, where the platform already handles back.
 */
export async function installBackButton(): Promise<void> {
  if (os !== 'android' || !('__TAURI_INTERNALS__' in window)) return;
  const { onBackButtonPress, exit } = await import('@tauri-apps/api/app');
  const bridge = window.MnemoraAndroid;
  const leave = bridge?.moveTaskToBack ? () => bridge.moveTaskToBack?.() : () => void exit(0);
  await onBackButtonPress((state) =>
    handleBack(state, { goBack: () => window.history.back(), leave }),
  );
}
