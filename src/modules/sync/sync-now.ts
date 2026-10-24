/** Said by the palette's "Sync now" when there is nobody to sync with (the top-bar button is disabled then). */
export const NO_DEVICES = 'No paired devices to sync with';

/** The palette's "Sync now": syncs with every reachable device, or says there are none paired. */
export async function runSyncNow(
  pairedCount: () => Promise<number>,
  syncNow: () => unknown,
  say: (message: string) => void,
) {
  if ((await pairedCount()) === 0) {
    say(NO_DEVICES);
    return;
  }
  void syncNow();
  say('Syncing with your devices');
}
