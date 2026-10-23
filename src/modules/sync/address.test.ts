import { describe, expect, it } from 'vitest';
import rules from '../../lib/ipc/fixtures/sync-rules.json';
import { ADDRESS_MESSAGES, parseAddress } from './address';

interface Case {
  name: string;
  input: string;
  stored?: string;
  port?: number;
  error?: string;
}

describe('parseAddress', () => {
  for (const c of rules.address_cases as Case[]) {
    it(c.name, () => {
      const got = parseAddress(c.input);
      if (c.error !== undefined) {
        expect(got).toEqual({ ok: false, message: c.error });
      } else {
        expect(got).toEqual({ ok: true, stored: c.stored, port: c.port });
      }
    });
  }

  it('says what to fix with the same words Rust uses', () => {
    expect(Object.values(ADDRESS_MESSAGES)).toEqual([
      'Type an address',
      "That isn't a host name or an IP address",
      'The port is a number from 1 to 65535',
      'Put an IPv6 address in square brackets, like [fd7a::1]',
    ]);
  });

  it('accepts a full-length IPv6 address and refuses nine groups', () => {
    expect(parseAddress('[fd7a:115c:a1e0:0:0:0:0:1]').ok).toBe(true);
    expect(parseAddress('[1:2:3:4:5:6:7:8:9]').ok).toBe(false);
  });
});
