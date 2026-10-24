import { describe, expect, it, vi } from 'vitest';
import { NO_DEVICES, runSyncNow } from './sync-now';

describe('the palette "Sync now"', () => {
  it('syncs and says so when a device is paired', async () => {
    const syncNow = vi.fn();
    const say = vi.fn();
    await runSyncNow(async () => 2, syncNow, say);
    expect(syncNow).toHaveBeenCalledOnce();
    expect(say).toHaveBeenCalledWith('Syncing with your devices');
  });

  it('with nothing paired it does nothing but say so, like the disabled top-bar button (A3)', async () => {
    const syncNow = vi.fn();
    const say = vi.fn();
    await runSyncNow(async () => 0, syncNow, say);
    expect(syncNow).not.toHaveBeenCalled();
    expect(say).toHaveBeenCalledWith(NO_DEVICES);
    expect(NO_DEVICES).toBe('No paired devices to sync with');
  });
});
