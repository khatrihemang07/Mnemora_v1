import { describe, expect, it } from 'vitest';
import { SAVE_PAUSE_MS } from './composer';
import { EntryEditing } from './entry-editing';

function setup(initial = 'hello', fails = false) {
  const calls: string[] = [];
  const saved: string[] = [];
  const errors: unknown[] = [];
  let timer: (() => void) | null = null;
  const state = { fails };
  const e = new EntryEditing<string>(
    {
      update: async (id, text, session) => {
        calls.push(`${id}:${text}:${session}`);
        if (state.fails) throw new Error('nope');
        return text;
      },
      setTimer: (fn, ms) => {
        expect(ms).toBe(SAVE_PAUSE_MS);
        timer = fn;
        return () => {
          timer = null;
        };
      },
      onSaved: (s) => saved.push(s),
      onError: (err) => errors.push(err),
    },
    'e1',
    initial,
    'sess',
  );
  const settle = () => new Promise((r) => setTimeout(r, 0));
  return {
    e,
    calls,
    saved,
    errors,
    state,
    fire: () => timer?.(),
    settle,
    hasTimer: () => timer !== null,
  };
}

describe('EntryEditing', () => {
  it('opening saves nothing', async () => {
    const { e, calls, hasTimer } = setup();
    expect(e.unsaved).toBe(false);
    expect(hasTimer()).toBe(false);
    expect(await e.flush()).toBe(true);
    expect(calls).toEqual([]);
  });

  it('saves after the pause with the session id, and restarts the pause on each key', async () => {
    const { e, calls, saved, fire, settle } = setup();
    e.input('hello w');
    e.input('hello world');
    fire();
    await settle();
    expect(calls).toEqual(['e1:hello world:sess']);
    expect(saved).toEqual(['hello world']);
    expect(e.unsaved).toBe(false);
  });

  it('typing back to the saved text cancels the pending save', () => {
    const { e, hasTimer } = setup();
    e.input('hello!');
    expect(hasTimer()).toBe(true);
    e.input('hello');
    expect(hasTimer()).toBe(false);
  });

  it('flush saves right away', async () => {
    const { e, calls } = setup();
    e.input('changed');
    expect(await e.flush()).toBe(true);
    expect(calls).toEqual(['e1:changed:sess']);
  });

  it('never saves blank text, and keeps the last saved text', async () => {
    const { e, calls } = setup();
    e.input('   ');
    expect(e.unsaved).toBe(false);
    expect(await e.flush()).toBe(true);
    expect(calls).toEqual([]);
  });

  it('a failed save keeps the text unsaved and reports it; the next edit retries', async () => {
    const { e, calls, errors, state, fire, settle } = setup('hello', true);
    e.input('hello again');
    expect(await e.flush()).toBe(false);
    expect(errors).toHaveLength(1);
    expect(e.unsaved).toBe(true);
    expect(e.text).toBe('hello again');

    state.fails = false;
    e.input('hello again!');
    fire();
    await settle();
    expect(calls.at(-1)).toBe('e1:hello again!:sess');
    expect(e.unsaved).toBe(false);
  });

  it('flush after a failure is a retry', async () => {
    const { e, state, saved } = setup('a', true);
    e.input('b');
    expect(await e.flush()).toBe(false);
    state.fails = false;
    expect(await e.flush()).toBe(true);
    expect(saved).toEqual(['b']);
  });

  it('reports blank text over a stored Entry as kept, never saved', async () => {
    const { e, calls } = setup('hello');
    expect(e.blankKept).toBe(false);
    e.input('  \n');
    expect(e.blankKept).toBe(true);
    expect(await e.flush()).toBe(true);
    expect(calls).toEqual([]);
    e.input('hello');
    expect(e.blankKept).toBe(false);
  });

  it('an abandoned session (the Entry was deleted elsewhere) saves nothing more and hands back its text', async () => {
    const { e, calls, hasTimer } = setup();
    e.input('typed after the delete');
    expect(hasTimer()).toBe(true);
    expect(e.abandon()).toBe('typed after the delete');
    expect(hasTimer()).toBe(false);
    expect(await e.flush()).toBe(true);
    expect(calls).toEqual([]);
  });
});
