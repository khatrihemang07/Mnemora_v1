import { describe, expect, it } from 'vitest';
import { SAVE_PAUSE_MS, TaskEditing } from './task-editing';

function setup(initial = { title: 'Pay rent', notes: '' }) {
  const calls: string[] = [];
  const timers: { fn: () => void; ms: number; live: boolean }[] = [];
  const saved: unknown[] = [];
  const errors: unknown[] = [];
  let fail: unknown = null;
  const editing = new TaskEditing<string>(
    {
      update: async (id, title, notes, session) => {
        calls.push(`${id}|${title}|${notes}|${session}`);
        if (fail) throw fail;
        return title;
      },
      setTimer: (fn, ms) => {
        const t = { fn, ms, live: true };
        timers.push(t);
        return () => {
          t.live = false;
        };
      },
      onSaved: (t) => saved.push(t),
      onError: (e) => errors.push(e),
    },
    't1',
    initial,
    's1',
  );
  const fire = () => {
    for (const t of timers.filter((t) => t.live)) {
      t.live = false;
      t.fn();
    }
  };
  return {
    editing,
    calls,
    saved,
    errors,
    timers,
    fire,
    failWith: (e: unknown) => {
      fail = e;
    },
  };
}

describe('TaskEditing', () => {
  it('saves after a pause with the session id, once for many keystrokes', async () => {
    const t = setup();
    t.editing.setNotes('h');
    t.editing.setNotes('he');
    t.editing.setNotes('hel');
    expect(t.timers.filter((x) => x.live)).toHaveLength(1);
    expect(t.timers[0]?.ms).toBe(SAVE_PAUSE_MS);
    t.fire();
    await t.editing.flush();
    expect(t.calls).toEqual(['t1|Pay rent|hel|s1']);
    expect(t.saved).toEqual(['Pay rent']);
  });

  it('saves title and notes together and trims the title', async () => {
    const t = setup();
    t.editing.setTitle('  Pay the rent ');
    t.editing.setNotes('on the 1st');
    await t.editing.flush();
    expect(t.calls).toEqual(['t1|Pay the rent|on the 1st|s1']);
  });

  it('saves nothing when the text matches what is stored', async () => {
    const t = setup();
    t.editing.setNotes('x');
    t.editing.setNotes('');
    expect(t.timers.filter((x) => x.live)).toHaveLength(0);
    await t.editing.flush();
    expect(t.calls).toEqual([]);
  });

  it('never saves a blank title: the notes go with the stored title, and leaving restores it', async () => {
    const t = setup();
    t.editing.setTitle('   ');
    expect(t.editing.blankTitle).toBe(true);
    expect(t.editing.unsaved).toBe(false);
    t.editing.setNotes('kept');
    await t.editing.flush();
    expect(t.calls).toEqual(['t1|Pay rent|kept|s1']);
    expect(t.editing.leaveTitle()).toBe('Pay rent');
    expect(t.editing.title).toBe('Pay rent');
    expect(t.editing.blankTitle).toBe(false);
  });

  it('keeps a typed title when leaving', () => {
    const t = setup();
    t.editing.setTitle('Pay the rent');
    expect(t.editing.leaveTitle()).toBe('Pay the rent');
  });

  it('folds pasted line breaks into spaces', () => {
    const t = setup();
    expect(t.editing.setTitle('one\ntwo\r\n three')).toBe('one two three');
  });

  it('keeps the text after a failed save and tries again on the next edit', async () => {
    const t = setup();
    t.failWith(new Error('disk'));
    t.editing.setNotes('a');
    expect(await t.editing.flush()).toBe(false);
    expect(t.errors).toHaveLength(1);
    expect(t.editing.unsaved).toBe(true);
    t.failWith(null);
    t.editing.setNotes('ab');
    expect(await t.editing.flush()).toBe(true);
    expect(t.calls.at(-1)).toBe('t1|Pay rent|ab|s1');
  });

  it('takes outside changes as stored', async () => {
    const t = setup({ title: 'Water', notes: '- [x] a' });
    t.editing.adopt({ title: 'Water', notes: '- [ ] a' });
    expect(t.editing.notes).toBe('- [ ] a');
    expect(t.editing.unsaved).toBe(false);
    await t.editing.flush();
    expect(t.calls).toEqual([]);
  });

  it('runs saves one after another, each sending the latest text', async () => {
    const t = setup();
    t.editing.setNotes('a');
    const first = t.editing.flush();
    t.editing.setNotes('ab');
    const second = t.editing.flush();
    await Promise.all([first, second]);
    expect(t.calls).toEqual(['t1|Pay rent|ab|s1']);
  });
});
