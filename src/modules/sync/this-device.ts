import type { ThisDevice } from '../../lib/ipc/bindings';

/** The line under This device's name, and what clicking it reveals. */
export interface ListeningSummary {
  listening: boolean;
  text: string;
  /** The addresses and port, for debugging; empty while not listening. */
  detail: string;
}

export function listeningSummary(device: ThisDevice): ListeningSummary {
  // Nothing is listening on purpose: the phone is on mobile data and Settings says not to use it.
  if (device.paused_on_mobile_data) {
    return { listening: false, text: 'Paused on mobile data', detail: '' };
  }
  if (device.port === null) {
    return { listening: false, text: 'Not listening yet', detail: '' };
  }
  const parts = [
    ...device.addresses.map((a) => (a.includes(':') ? `[${a}]` : a)),
    `port ${device.port}`,
  ];
  // U+2011 is a hyphen that never breaks the line ("Wi-" / "Fi" on a phone).
  const networks = device.routes.map((r) => (r === 'wifi' ? 'Wi\u2011Fi' : 'Tailscale'));
  return {
    listening: true,
    text: networks.length > 0 ? `Listening on ${networks.join(' and ')}` : 'Listening',
    detail: parts.join(' · '),
  };
}
