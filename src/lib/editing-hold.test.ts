import { describe, expect, it, vi } from 'vitest';
import { EditingHold, HOLD_RENEW_MS } from './editing-hold';

function setup(opts: { failHold?: boolean; failRelease?: boolean } = {}) {
  const calls: string[] = [];
  const timers = new Map<number, () => void>();
  let next = 0;
  const hold = new EditingHold({
    hold: async (scope) => {
      calls.push(`hold ${scope}`);
      if (opts.failHold) throw new Error('down');
    },
    release: async (scope) => {
      calls.push(`release ${scope}`);
      if (opts.failRelease) throw new Error('down');
    },
    every: (fn, ms) => {
      expect(ms).toBe(HOLD_RENEW_MS);
      const id = next++;
      timers.set(id, fn);
      return () => timers.delete(id);
    },
  });
  return { hold, calls, timers };
}

describe('EditingHold', () => {
  it('holds at once, renews while open and lets go when told', async () => {
    const { hold, calls, timers } = setup();
    hold.take('*');
    expect(calls).toEqual(['hold *']);
    for (const tick of timers.values()) tick();
    expect(calls).toEqual(['hold *', 'hold *']);
    await hold.release('*');
    expect(calls).toEqual(['hold *', 'hold *', 'release *']);
    expect(timers.size).toBe(0);
    expect(hold.has('*')).toBe(false);
  });

  it('takes a scope once however often it is asked (every keystroke asks)', () => {
    const { hold, calls, timers } = setup();
    hold.take('title');
    hold.take('title');
    hold.take('title');
    expect(calls).toEqual(['hold title']);
    expect(timers.size).toBe(1);
  });

  it('releases only what it holds, and everything on releaseAll', async () => {
    const { hold, calls } = setup();
    await hold.release('notes');
    expect(calls).toEqual([]);
    hold.take('title');
    hold.take('notes');
    await hold.releaseAll();
    expect(calls.filter((c) => c.startsWith('release')).sort()).toEqual([
      'release notes',
      'release title',
    ]);
    expect(hold.has('title') || hold.has('notes')).toBe(false);
  });

  it('survives Rust refusing: a failed hold or release is ignored', async () => {
    const { hold } = setup({ failHold: true, failRelease: true });
    const unhandled = vi.fn();
    process.on('unhandledRejection', unhandled);
    hold.take('*');
    await expect(hold.release('*')).resolves.toBeUndefined();
    await new Promise((r) => setTimeout(r, 0));
    process.off('unhandledRejection', unhandled);
    expect(unhandled).not.toHaveBeenCalled();
  });
});
