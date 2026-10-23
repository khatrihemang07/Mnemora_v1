/**
 * The id of the occurrence copy of a repeating Task, derived the way crates/todo/src/repeat.rs
 * `occurrence_id` does: UUIDv5 over "{task_id}|{due}" in the pinned namespace, so the mock gives
 * the same id on every device and in every database.
 */

/** Mirrors `OCCURRENCE_NAMESPACE` in repeat.rs; never changes. */
const NAMESPACE = '98e73eb4-587f-441e-9d52-017d09dac720';

/** SHA-1 of `bytes` (RFC 3174), synchronously: the mock's store is synchronous. */
function sha1(bytes: Uint8Array): Uint8Array {
  const words = new Uint32Array((((bytes.length + 8) >> 6) + 1) * 16);
  for (let i = 0; i < bytes.length; i++)
    words[i >> 2] = (words[i >> 2] as number) | ((bytes[i] as number) << (24 - 8 * (i & 3)));
  words[bytes.length >> 2] =
    (words[bytes.length >> 2] as number) | (0x80 << (24 - 8 * (bytes.length & 3)));
  words[words.length - 1] = (bytes.length * 8) >>> 0;
  words[words.length - 2] = Math.floor((bytes.length * 8) / 2 ** 32);
  const h = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0];
  const w = new Uint32Array(80);
  const rotl = (x: number, n: number) => ((x << n) | (x >>> (32 - n))) >>> 0;
  for (let block = 0; block < words.length; block += 16) {
    for (let i = 0; i < 16; i++) w[i] = words[block + i] as number;
    for (let i = 16; i < 80; i++)
      w[i] = rotl(
        (w[i - 3] as number) ^ (w[i - 8] as number) ^ (w[i - 14] as number) ^ (w[i - 16] as number),
        1,
      );
    let [a, b, c, d, e] = h as [number, number, number, number, number];
    for (let i = 0; i < 80; i++) {
      const [f, k] =
        i < 20
          ? [(b & c) | (~b & d), 0x5a827999]
          : i < 40
            ? [b ^ c ^ d, 0x6ed9eba1]
            : i < 60
              ? [(b & c) | (b & d) | (c & d), 0x8f1bbcdc]
              : [b ^ c ^ d, 0xca62c1d6];
      const next = (rotl(a, 5) + (f as number) + e + (k as number) + (w[i] as number)) >>> 0;
      e = d;
      d = c;
      c = rotl(b, 30);
      b = a;
      a = next;
    }
    h[0] = ((h[0] as number) + a) >>> 0;
    h[1] = ((h[1] as number) + b) >>> 0;
    h[2] = ((h[2] as number) + c) >>> 0;
    h[3] = ((h[3] as number) + d) >>> 0;
    h[4] = ((h[4] as number) + e) >>> 0;
  }
  const out = new Uint8Array(20);
  h.forEach((x, i) => {
    out[i * 4] = x >>> 24;
    out[i * 4 + 1] = (x >>> 16) & 255;
    out[i * 4 + 2] = (x >>> 8) & 255;
    out[i * 4 + 3] = x & 255;
  });
  return out;
}

const hex = (bytes: Uint8Array) => [...bytes].map((b) => b.toString(16).padStart(2, '0')).join('');

/** The id of the occurrence copy of `taskId` due on `due` (`YYYY-MM-DD`). */
export function occurrenceId(taskId: string, due: string): string {
  const ns =
    NAMESPACE.replaceAll('-', '')
      .match(/../g)
      ?.map((h) => Number.parseInt(h, 16)) ?? [];
  const name = new TextEncoder().encode(`${taskId}|${due}`);
  const digest = sha1(Uint8Array.from([...ns, ...name])).slice(0, 16);
  digest[6] = ((digest[6] as number) & 0x0f) | 0x50;
  digest[8] = ((digest[8] as number) & 0x3f) | 0x80;
  const h = hex(digest);
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
}
