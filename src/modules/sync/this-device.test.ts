import { describe, expect, it } from 'vitest';
import type { ThisDevice } from '../../lib/ipc/bindings';
import { listeningSummary } from './this-device';

const base: ThisDevice = {
  name: 'Mac',
  platform: 'desktop',
  port: 47311,
  addresses: [],
  routes: [],
  paused_on_mobile_data: false,
  local_network_blocked: false,
};

describe('listeningSummary', () => {
  it('says Listening on Wi-Fi and Tailscale, with the addresses and port behind it', () => {
    const s = listeningSummary({
      ...base,
      addresses: ['192.168.1.20', '100.101.4.7'],
      routes: ['wifi', 'tailscale'],
    });
    expect(s.listening).toBe(true);
    expect(s.text).toBe('Listening on Wi\u2011Fi and Tailscale');
    expect(s.detail).toBe('192.168.1.20 · 100.101.4.7 · port 47311');
  });

  it('says why it is not listening on mobile data when the setting is off', () => {
    const s = listeningSummary({ ...base, port: null, paused_on_mobile_data: true });
    expect(s).toEqual({ listening: false, text: 'Paused on mobile data', detail: '' });
    expect(listeningSummary({ ...base, port: null }).text).toBe('Not listening yet');
  });

  it('names only the networks that are really there', () => {
    const only = (routes: ThisDevice['routes']) =>
      listeningSummary({ ...base, addresses: ['x'], routes }).text;
    expect(only(['wifi'])).toBe('Listening on Wi\u2011Fi');
    expect(only(['tailscale'])).toBe('Listening on Tailscale');
    expect(only([])).toBe('Listening');
  });

  it('brackets an IPv6 address', () => {
    expect(listeningSummary({ ...base, addresses: ['fd7a:115c:a1e0::1'] }).detail).toBe(
      '[fd7a:115c:a1e0::1] · port 47311',
    );
  });

  it('says so when nothing is listening', () => {
    const s = listeningSummary({ ...base, port: null });
    expect(s).toEqual({ listening: false, text: 'Not listening yet', detail: '' });
  });
});
