import { beforeEach, describe, expect, it, vi } from 'vitest';

const enterAt = vi.fn(async () => ({ status: 'ok', data: null }));
// Deny: the other device's view change arrives while the command is still being answered.
const decide = vi.fn(
  async (_allow: boolean): Promise<{ status: 'ok'; data: null }> => ({
    status: 'ok',
    data: null,
  }),
);
const show = vi.fn(async () => ({ status: 'ok', data: null }));
const onMobile = { paused: false };
vi.mock('../../lib/ui/toast', () => ({ notify: vi.fn() }));
vi.mock('../../lib/ipc/bindings', () => ({
  commands: {
    syncPairEnterAt: (...a: unknown[]) => enterAt(...(a as [])),
    syncPairDecide: (allow: boolean) => decide(allow),
    syncPairShow: () => show(),
    syncDevices: async () => ({
      status: 'ok',
      data: { this_device: { paused_on_mobile_data: onMobile.paused }, peers: [] },
    }),
  },
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

describe('editing after a failure', () => {
  const failed = {
    state: 'failed',
    error: { kind: 'address_unreachable' },
  } as const;

  it('clears the failure banner as soon as the address or the code is edited', () => {
    pairing.view = failed;
    pairing.editAddress('mac.tail1234.ts.net');
    expect(pairing.view.state).toBe('idle');
    pairing.view = failed;
    pairing.editCode('1234');
    expect(pairing.view.state).toBe('idle');
  });

  it('leaves other views alone', () => {
    pairing.view = { state: 'searching' } as never;
    pairing.editCode('1234');
    expect((pairing.view as { state: string }).state).toBe('searching');
  });
});

describe('denying', () => {
  it('closes the sheet before the answer comes back, so it never shows an empty step', async () => {
    pairing.open = true;
    pairing.view = { state: 'asking', name: 'Pixel 8' } as never;
    let openWhenAnswered: boolean | null = null;
    decide.mockImplementationOnce(async () => {
      openWhenAnswered = pairing.open;
      return { status: 'ok', data: null };
    });
    await pairing.decide(false);
    expect(openWhenAnswered).toBe(false);
    expect(pairing.open).toBe(false);
  });
});

describe('on mobile data with "Allow over mobile data" off', () => {
  beforeEach(() => {
    show.mockClear();
    onMobile.paused = true;
  });

  it('explains up front and shows no code', async () => {
    await pairing.start('show');
    expect(pairing.open).toBe(true);
    expect(pairing.pausedOnMobile).toBe(true);
    expect(show).not.toHaveBeenCalled();
  });

  it('shows the code once mobile data is no longer the reason', async () => {
    await pairing.start('show');
    onMobile.paused = false;
    await pairing.start('show');
    expect(pairing.pausedOnMobile).toBe(false);
    expect(show).toHaveBeenCalledOnce();
  });
});
