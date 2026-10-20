import { os } from '../platform';

/**
 * Android hardware back: go back in history, or exit at the root. A no-op on desktop and in a
 * browser, where the platform already handles back.
 */
export async function installBackButton(): Promise<void> {
  if (os !== 'android' || !('__TAURI_INTERNALS__' in window)) return;
  const { onBackButtonPress, exit } = await import('@tauri-apps/api/app');
  await onBackButtonPress(({ canGoBack }) => {
    if (canGoBack) window.history.back();
    else void exit(0);
  });
}
