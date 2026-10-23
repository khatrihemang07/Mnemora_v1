import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { IpcError } from '../../lib/ipc/unwrap';
import { urlParam } from '../../lib/router/url-param';
import { deviceGone } from './device-detail';
import { deviceSheet } from './device-sheet.svelte';

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

describe('the device sheet in the URL', () => {
  const back = vi.fn();
  beforeEach(() => {
    vi.useFakeTimers();
    vi.stubGlobal('history', { back });
    deviceSheet.dismiss();
    back.mockClear();
    vi.advanceTimersByTime(1000);
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('opens from a link: the parameter is the device', () => {
    urlParam.set('device', 'pixel');
    expect(deviceSheet.id).toBe('pixel');
  });

  it('closing a sheet opened on a link drops the parameter instead of leaving the page', () => {
    urlParam.set('device', 'pixel');
    deviceSheet.close();
    expect(deviceSheet.id).toBeNull();
    expect(back).not.toHaveBeenCalled();
  });

  it('closing a sheet opened by a click steps back over the entry the click pushed', () => {
    deviceSheet.open('pixel');
    expect(deviceSheet.id).toBe('pixel');
    deviceSheet.close();
    expect(back).toHaveBeenCalledOnce();
  });

  it('two closes within a moment are one', () => {
    deviceSheet.open('pixel');
    deviceSheet.close();
    deviceSheet.close();
    expect(back).toHaveBeenCalledOnce();
  });

  it('dismissing a device that is gone drops the parameter without stepping back', () => {
    deviceSheet.open('pixel');
    deviceSheet.dismiss();
    expect(deviceSheet.id).toBeNull();
    expect(back).not.toHaveBeenCalled();
  });
});

describe('deviceGone', () => {
  it('is true for a device that is not paired and for nothing else', () => {
    const error = (code: 'not_found' | 'internal') =>
      new IpcError({ code, message: 'x', details: null });
    expect(deviceGone(error('not_found'))).toBe(true);
    expect(deviceGone(error('internal'))).toBe(false);
    expect(deviceGone(new Error('x'))).toBe(false);
    expect(deviceGone(null)).toBe(false);
  });
});
