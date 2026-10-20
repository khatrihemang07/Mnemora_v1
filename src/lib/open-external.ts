import { inTauriWindow } from './platform';

/** Opens a link in the system browser (a new tab in the dev browser). */
export async function openExternal(url: string): Promise<void> {
  if (!inTauriWindow()) {
    window.open(url, '_blank', 'noopener,noreferrer');
    return;
  }
  const { openUrl } = await import('@tauri-apps/plugin-opener');
  await openUrl(url);
}
