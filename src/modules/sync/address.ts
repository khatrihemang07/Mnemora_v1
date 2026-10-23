/**
 * The Address a person may type for a device: a MagicDNS name, a host, `host:port`, an IPv4
 * address or an IPv6 address in square brackets. The same rules as `crates/sync/src/address.rs`
 * (both read `src/lib/ipc/fixtures/sync-rules.json`). Rust checks it again on save; this is for
 * the message under the field while typing, and for the browser mock.
 */

export const DEFAULT_PORT = 47311;

export const ADDRESS_MESSAGES = {
  empty: 'Type an address',
  host: "That isn't a host name or an IP address",
  port: 'The port is a number from 1 to 65535',
  brackets: 'Put an IPv6 address in square brackets, like [fd7a::1]',
} as const;

export type AddressResult =
  | { ok: true; stored: string; port: number }
  | { ok: false; message: string };

const fail = (message: string): AddressResult => ({ ok: false, message });

function parsePort(text: string): number | null {
  if (!/^[0-9]+$/.test(text)) return null;
  const port = Number(text);
  return port >= 1 && port <= 65535 ? port : null;
}

function isIpv4(text: string): boolean {
  const parts = text.split('.');
  return (
    parts.length === 4 &&
    parts.every(
      (p) => /^[0-9]{1,3}$/.test(p) && (p === '0' || !p.startsWith('0')) && Number(p) <= 255,
    )
  );
}

/** An IPv6 reader: groups of 1 to 4 hex digits, at most one `::` (no embedded IPv4). */
function isIpv6(text: string): boolean {
  if (!/^[0-9a-fA-F:]+$/.test(text) || !text.includes(':') || text.includes(':::')) return false;
  const halves = text.split('::');
  if (halves.length > 2) return false;
  const parts = halves.flatMap((half) => (half === '' ? [] : half.split(':')));
  if (parts.some((g) => g.length < 1 || g.length > 4)) return false;
  return halves.length === 2 ? parts.length <= 7 : parts.length === 8;
}

function hostName(text: string): string | null {
  const name = asciiLower(text.endsWith('.') ? text.slice(0, -1) : text);
  if (name === '' || name.length > 253 || /^[0-9.]+$/.test(name)) return null;
  const label = /^[a-z0-9_]([a-z0-9_-]{0,61}[a-z0-9_])?$/;
  return name.split('.').every((l) => label.test(l)) ? name : null;
}

/** Reads what was typed: the text to store, or what to tell the person. */
export function parseAddress(raw: string): AddressResult {
  const text = raw.trim();
  if (text === '') return fail(ADDRESS_MESSAGES.empty);
  if (text.startsWith('[')) {
    const close = text.indexOf(']');
    if (close < 0) return fail(ADDRESS_MESSAGES.host);
    const inner = text.slice(1, close);
    if (!isIpv6(inner)) return fail(ADDRESS_MESSAGES.host);
    const after = text.slice(close + 1);
    let port: number | null = null;
    if (after !== '') {
      if (!after.startsWith(':')) return fail(ADDRESS_MESSAGES.host);
      port = parsePort(after.slice(1));
      if (port === null) return fail(ADDRESS_MESSAGES.port);
    }
    return {
      ok: true,
      stored: `[${normalizeIpv6(inner)}]${port === null ? '' : `:${port}`}`,
      port: port ?? DEFAULT_PORT,
    };
  }
  if (isIpv6(text)) return fail(ADDRESS_MESSAGES.brackets);
  // Two colons, or a scheme (`http://…`): not a host with a port.
  const colons = text.split(':').length - 1;
  if (colons > 1 || text.includes('://')) return fail(ADDRESS_MESSAGES.host);
  const [host = '', portText] = text.split(':');
  let port: number | null = null;
  if (portText !== undefined) {
    port = parsePort(portText);
    if (port === null) return fail(ADDRESS_MESSAGES.port);
  }
  const name = isIpv4(host) ? host : hostName(host);
  if (name === null) return fail(ADDRESS_MESSAGES.host);
  return { ok: true, stored: port === null ? name : `${name}:${port}`, port: port ?? DEFAULT_PORT };
}

/** Lower-cases ASCII letters only, as Rust does (`toLowerCase` also turns the Kelvin sign into a `k`). */
function asciiLower(text: string): string {
  return text.replace(/[A-Z]/g, (c) => c.toLowerCase());
}

/**
 * An IPv6 address the way Rust prints it: lower case, no leading zeros, and the longest run of
 * zero groups (two or more; the first of equals) written `::`. `text` passed `isIpv6`.
 */
function normalizeIpv6(text: string): string {
  const [head = '', tail] = text.split('::');
  const groups = (part: string) => (part === '' ? [] : part.split(':'));
  const left = groups(head);
  const right = tail === undefined ? [] : groups(tail);
  const zeros = tail === undefined ? [] : Array<string>(8 - left.length - right.length).fill('0');
  const all = [...left, ...zeros, ...right].map((g) => Number.parseInt(g, 16));
  let best = { start: -1, length: 0 };
  for (let i = 0; i < all.length; ) {
    if (all[i] !== 0) {
      i += 1;
      continue;
    }
    let end = i;
    while (end < all.length && all[end] === 0) end += 1;
    if (end - i > best.length) best = { start: i, length: end - i };
    i = end;
  }
  const hex = (n: number) => n.toString(16);
  if (best.length < 2) return all.map(hex).join(':');
  const before = all.slice(0, best.start).map(hex).join(':');
  const after = all
    .slice(best.start + best.length)
    .map(hex)
    .join(':');
  return `${before}::${after}`;
}
