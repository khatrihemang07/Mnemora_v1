import { beforeEach, describe, expect, it, vi } from 'vitest';

const enterAt = vi.fn(async () => ({ status: 'ok', data: null }));
vi.mock('../../lib/ipc/bindings', () => ({
  commands: { syncPairEnterAt: (...a: unknown[]) => enterAt(...(a as [])) },
  events: {},
}));
vi.mock('./queries', () => ({ refreshSync: vi.fn() }));

const { pairing } = await import('./pairing.svelte');

describe('pairing by address', () => {
  beforeEach(() => {
    enterAt.mockClear();
    pairing.address = '';
    pairing.code = '12345678';
    pairing.addressError = null;
  });

  it('refuses an address that is not a host name before dialling anything', async () => {
    pairing.address = 'bad address!!';
    await pairing.submitAddress();
    expect(pairing.addressError).toBe('That isn’t a host name or an IP address');
    expect(enterAt).not.toHaveBeenCalled();
  });

  it('says so even while the code is not complete', async () => {
    pairing.address = 'bad address!!';
    pairing.code = '123';
    await pairing.submitAddress();
    expect(pairing.addressError).toBe('That isn’t a host name or an IP address');
    expect(enterAt).not.toHaveBeenCalled();
  });

  it('dials a good address with the code, and leaves an empty field quiet', async () => {
    expect(pairing.checkAddress()).toBe(false);
    expect(pairing.addressError).toBeNull();
    pairing.address = 'mac.tail1234.ts.net';
    await pairing.submitAddress();
    expect(pairing.addressError).toBeNull();
    expect(enterAt).toHaveBeenCalledWith('mac.tail1234.ts.net', '12345678');
  });
});
