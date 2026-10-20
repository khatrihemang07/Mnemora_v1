import { describe, expect, it } from 'vitest';
import { Composer, SAVE_PAUSE_MS } from './composer';

function setup(opts: { failCreate?: boolean; failUpdate?: boolean } = {}) {
  const calls: string[] = [];
  const errors: unknown[] = [];
  let timer: (() => void) | null = null;
  let sessions = 0;
  const c = new Composer({
    create: async (text, session) => {
      calls.push(`create:${text}:${session}`);
      if (opts.failCreate) throw new Error('nope');
      return { id: 'e1' };
    },
    update: async (id, text, session) => {
      calls.push(`update:${id}:${text}:${session}`);
      if (opts.failUpdate) throw new Error('nope');
    },
    setTimer: (fn, ms) => {
      expect(ms).toBe(SAVE_PAUSE_MS);
      timer = fn;
      return () => {
        timer = null;
      };
    },
    newSessionId: () => `s${++sessions}`,
    onError: (e) => errors.push(e),
  });
  const settle = () => new Promise((r) => setTimeout(r, 0));
  return { c, calls, errors, fire: () => timer?.(), settle, hasTimer: () => timer !== null };
}

describe('Composer', () => {
  it('never creates from blank text', async () => {
    const { c, calls, settle } = setup();
    c.input('');
    c.input('   \n ');
    await settle();
    expect(calls).toEqual([]);
    expect(await c.finish()).toBe(true);
    expect(calls).toEqual([]);
  });

  it('creates once on the first visible character, then updates after the pause', async () => {
    const { c, calls, fire, settle, hasTimer } = setup();
    c.input('h');
    c.input('he');
    await settle();
    expect(c.phase).toBe('editing');
    expect(calls).toEqual(['create:h:s1']);
    // typed while creating: saved after the pause
    expect(hasTimer()).toBe(true);
    fire();
    await settle();
    expect(calls).toEqual(['create:h:s1', 'update:e1:he:s1']);
    c.input('hel');
    c.input('hell');
    fire();
    await settle();
    expect(calls.slice(2)).toEqual(['update:e1:hell:s1']);
  });

  it('finishing flushes the pending text, clears, and starts a new session', async () => {
    const { c, calls, settle } = setup();
    c.input('a');
    await settle();
    c.input('a b');
    expect(await c.finish()).toBe(true);
    expect(calls).toEqual(['create:a:s1', 'update:e1:a b:s1']);
    expect(c.text).toBe('');
    expect(c.phase).toBe('idle');
    c.input('next');
    await settle();
    expect(calls[2]).toBe('create:next:s2');
  });

  it('finishing right after the first character waits for the create', async () => {
    const { c, calls } = setup();
    c.input('x');
    c.input('xy');
    expect(await c.finish()).toBe(true);
    expect(calls).toEqual(['create:x:s1', 'update:e1:xy:s1']);
  });

  it('a failed create keeps the text, reports, and retries on the next keystroke', async () => {
    const { c, errors, settle } = setup({ failCreate: true });
    c.input('x');
    await settle();
    expect(errors).toHaveLength(1);
    expect(c.phase).toBe('idle');
    expect(c.text).toBe('x');
    expect(await c.finish()).toBe(false);
    expect(c.text).toBe('x');
  });

  it('a failed update keeps the text on finish', async () => {
    const { c, errors, settle } = setup({ failUpdate: true });
    c.input('x');
    await settle();
    c.input('xy');
    expect(await c.finish()).toBe(false);
    expect(errors).toHaveLength(1);
    expect(c.text).toBe('xy');
  });
});
