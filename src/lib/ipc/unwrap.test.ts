import { describe, expect, it } from 'vitest';
import { IpcError, unwrap } from './unwrap';

describe('unwrap', () => {
  it('returns data on ok', async () => {
    await expect(unwrap(Promise.resolve({ status: 'ok' as const, data: 5 }))).resolves.toBe(5);
  });
  it('throws a typed error on failure', async () => {
    const error = { code: 'validation' as const, message: 'bad', details: 'why' };
    const err = await unwrap(Promise.resolve({ status: 'error' as const, error })).catch((e) => e);
    expect(err).toBeInstanceOf(IpcError);
    expect(err).toMatchObject({ code: 'validation', message: 'bad', details: 'why' });
  });
});
