import { describe, expect, it, vi } from 'vitest';
import { EditingHold, HOLD_RENEW_MS } from './editing-hold';

const settle = () => new Promise((r) => setTimeout(r, 0));

function setup(opts: { failHold?: boolean; failRelease?: boolean; syncOn?: boolean } = {}) {
  let visible = true;
  const listeners = new Set<() => void>();
  const calls: string[] = [];
  const timers = new Map<number, () => void>();
  let next = 0;
  const hold = new EditingHold({
    hold: async (scope) => {
      calls.push(`hold ${scope}`);
      if (opts.failHold) throw new Error('down');
      return opts.syncOn ?? true;
    },
    release: async (scope) => {
      calls.push(`release ${scope}`);
      if (opts.failRelease) throw new Error('down');
    },
    repeat: (fn, ms) => {
      expect(ms).toBe(HOLD_RENEW_MS);
      const id = next++;
      timers.set(id, fn);
      return () => timers.delete(id);
    },
    visible: () => visible,
    onShown: (fn) => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
  });
  return {
    hold,
    calls,
    timers,
    listeners,
    show: (v: boolean) => {
      visible = v;
      if (v) for (const fn of listeners) fn();
    },
  };
}

describe('EditingHold', () => {
  it('holds at once, renews while open and lets go when told', async () => {
    const { hold, calls, timers } = setup();
    hold.take('*');
    expect(calls).toEqual(['hold *']);
    await settle();
    for (const tick of timers.values()) tick();
    expect(calls).toEqual(['hold *', 'hold *']);
    await hold.release('*');
    expect(calls).toEqual(['hold *', 'hold *', 'release *']);
    expect(timers.size).toBe(0);
    expect(hold.has('*')).toBe(false);
  });

  it('takes a scope once however often it is asked (every keystroke asks)', async () => {
    const { hold, calls, timers } = setup();
    hold.take('title');
    hold.take('title');
    hold.take('title');
    expect(calls).toEqual(['hold title']);
    await settle();
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

  it('starts no timer when Sync is off, and calls Rust only once', async () => {
    const { hold, calls, timers } = setup({ syncOn: false });
    hold.take('*');
    hold.take('*');
    await settle();
    expect(timers.size).toBe(0);
    expect(calls).toEqual(['hold *']);
    await hold.release('*');
    expect(calls).toEqual(['hold *', 'release *']);
  });

  it('starts no timer when released before Rust answered', async () => {
    const { hold, timers } = setup();
    hold.take('*');
    const done = hold.release('*');
    await done;
    await settle();
    expect(timers.size).toBe(0);
  });

  it('does not renew while the window is hidden, and renews at once when shown', async () => {
    const { hold, calls, timers, show, listeners } = setup();
    hold.take('*');
    await settle();
    show(false);
    for (const tick of timers.values()) tick();
    expect(calls).toEqual(['hold *']);
    show(true);
    expect(calls).toEqual(['hold *', 'hold *']);
    await hold.release('*');
    expect(listeners.size).toBe(0);
  });
});
