import type { ThisDevice } from '../../lib/ipc/bindings';

/** The line under This device's name, and what clicking it reveals. */
export interface ListeningSummary {
  listening: boolean;
  text: string;
  /** The addresses and port, for debugging; empty while not listening. */
  detail: string;
}

export function listeningSummary(device: ThisDevice): ListeningSummary {
  if (device.port === null) {
    return { listening: false, text: 'Not listening yet', detail: '' };
  }
  const parts = [
    ...device.addresses.map((a) => (a.includes(':') ? `[${a}]` : a)),
    `port ${device.port}`,
  ];
  return { listening: true, text: 'Listening on Wi-Fi and Tailscale', detail: parts.join(' · ') };
}
