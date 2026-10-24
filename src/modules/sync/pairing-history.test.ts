import { beforeEach, describe, expect, it, vi } from 'vitest';

const cancel = vi.fn(async () => ({ status: 'ok', data: null }));
vi.mock('../../lib/ui/toast', () => ({ notify: vi.fn() }));
vi.mock('../../lib/ipc/bindings', () => ({
  commands: {
    syncPairShow: async () => ({ status: 'ok', data: null }),
    syncPairCancel: () => cancel(),
    syncPairDecide: async () => ({ status: 'ok', data: null }),
    syncPairMerge: async () => ({ status: 'ok', data: null }),
    syncDevices: async () => ({
      status: 'ok',
      data: { this_device: { paused_on_mobile_data: false }, peers: [] },
    }),
  },
  events: {},
}));
vi.mock('./queries', () => ({ refreshSync: vi.fn() }));
vi.mock('../../lib/router/url-param', async () => {
  const { SvelteURLSearchParams } = await import('svelte/reactivity');
  const params = new SvelteURLSearchParams();
  return {
    urlParam: {
      get: (name: string) => params.get(name),
      set: (name: string, value: string) => params.set(name, value),
      delete: (name: string) => params.delete(name),
    },
  };
});

const { pairing } = await import('./pairing.svelte');
const { urlParam } = await import('../../lib/router/url-param');
const { deviceSheet } = await import('./device-sheet.svelte');

// Back with the Pair sheet open closes it, as Back closes the device sheet (R8). Android's hardware
// Back is the Sheet's own; this is the history entry that browser Back and the URL follow.
describe('the Pair sheet and Back', () => {
  const back = vi.fn(() => urlParam.delete('pair', true));
  beforeEach(async () => {
    vi.stubGlobal('history', { back });
    back.mockClear();
    cancel.mockClear();
    pairing.open = false;
    await pairing.close(false);
    back.mockClear();
  });

  it('opening adds one history entry, and switching between show and enter adds no more', async () => {
    await pairing.start('show');
    expect(urlParam.get('pair')).toBe('show');
    await pairing.switchTo('enter');
    expect(pairing.open).toBe(true);
    await pairing.close();
    expect(back).toHaveBeenCalledOnce();
  });

  it('browser Back closes the sheet and ends the pairing without stepping back again', async () => {
    await pairing.start('show');
    urlParam.delete('pair'); // what Back does to the URL
    pairing.backed();
    await vi.waitFor(() => expect(cancel).toHaveBeenCalled());
    expect(pairing.open).toBe(false);
    expect(back).not.toHaveBeenCalled();
  });

  it('Deny and Cancel on the merge preview step back over the entry too', async () => {
    await pairing.start('show');
    pairing.view = { state: 'asking', name: 'Pixel' } as never;
    await pairing.decide(false);
    expect(back).toHaveBeenCalledOnce();
    back.mockClear();
    await pairing.start('show');
    await pairing.merge(false);
    expect(back).toHaveBeenCalledOnce();
  });

  it('a sheet closed because the page was left keeps the history alone', async () => {
    await pairing.start('show');
    await pairing.close(false);
    expect(back).not.toHaveBeenCalled();
    expect(pairing.open).toBe(false);
  });
});

// N2: the other device forgot this one while its sheet for that device was open. The Devices page
// drops the sheet (dismiss) and shows the welcome card; every tap on Pair a device must then open the sheet.
describe('Pair a device after this device was forgotten (N2)', () => {
  const back = vi.fn(() => urlParam.delete('pair', true));
  beforeEach(() => {
    vi.stubGlobal('history', { back });
    back.mockClear();
  });

  it('opens on every tap, however the device sheet and the last Pair sheet ended', async () => {
    deviceSheet.open('mac');
    deviceSheet.dismiss();
    expect(deviceSheet.id).toBeNull();
    for (let tap = 0; tap < 5; tap++) {
      await pairing.start('show');
      expect(pairing.open).toBe(true);
      expect(urlParam.get('pair')).toBe('show');
      // Every second sheet is left open, as when the person taps again behind it.
      if (tap % 2 === 0) await pairing.close();
    }
    await pairing.close();
    await pairing.start('show');
    expect(pairing.open).toBe(true);
  });
});
