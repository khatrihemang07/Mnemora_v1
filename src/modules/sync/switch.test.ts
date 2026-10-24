import { beforeEach, describe, expect, it, vi } from 'vitest';

const device = { data: { sync: { enabled: false } }, isSuccess: true };
let finishEnable: () => void = () => {};
const setEnabled = vi.fn(
  () =>
    new Promise((resolve) => {
      finishEnable = () => resolve({ status: 'ok', data: null });
    }),
);
vi.mock('@tanstack/svelte-query', () => ({ createQuery: () => device }));
vi.mock('../../lib/ipc/bindings', () => ({ commands: { syncSetEnabled: () => setEnabled() } }));
vi.mock('../../lib/settings/queries', () => ({ deviceQuery: {} }));
vi.mock('../../lib/ui/toast', () => ({ notifyError: vi.fn() }));
vi.mock('./local-network', () => ({ askLocalNetworkOnEnable: vi.fn() }));
vi.mock('./queries', () => ({ refreshSync: vi.fn(async () => {}) }));

const { turnOffDialog, useSyncSwitch } = await import('./switch.svelte');

describe('the Modules switch for Sync (R14)', () => {
  const navigate = vi.fn();
  beforeEach(() => {
    setEnabled.mockClear();
    navigate.mockClear();
    turnOffDialog.open = false;
    device.data.sync.enabled = false;
  });

  it('a triple click turns Sync on once, opens the page once and never asks to turn it off', async () => {
    const sw = useSyncSwitch({ navigate } as never);
    sw.set(true);
    // The second and third click land while the first is still being answered; the switch already reads on.
    device.data.sync.enabled = true;
    sw.set(false);
    sw.set(true);
    expect(turnOffDialog.open).toBe(false);
    finishEnable();
    await vi.waitFor(() => expect(navigate).toHaveBeenCalledOnce());
    expect(setEnabled).toHaveBeenCalledOnce();
    expect(turnOffDialog.open).toBe(false);
  });

  it('turning off still asks first once the switch has settled', async () => {
    const sw = useSyncSwitch({ navigate } as never);
    sw.set(true);
    finishEnable();
    await vi.waitFor(() => expect(navigate).toHaveBeenCalledOnce());
    device.data.sync.enabled = true;
    sw.set(false);
    expect(turnOffDialog.open).toBe(true);
  });
});
