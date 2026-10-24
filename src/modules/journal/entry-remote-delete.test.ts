import { describe, expect, it, vi } from 'vitest';
import type { JournalEntryDeletedEvent } from '../../lib/ipc/bindings';
import { followEntryDelete, remoteDeletedMessage, restoreWithDraft } from './entry-remote-delete';

const event: JournalEntryDeletedEvent = {
  entry_id: 'e1',
  page_date: '2026-10-21',
  device: 'Pixel 8',
};

describe('an Entry deleted on another device', () => {
  it('names the Page and the device', () => {
    expect(remoteDeletedMessage(event, '2026-10-24')).toBe('‘Wed 21 Oct’ was deleted on Pixel 8');
    expect(remoteDeletedMessage({ ...event, device: null }, '2026-10-24')).toBe(
      '‘Wed 21 Oct’ was deleted on another device',
    );
  });

  it('closes the editor only when that Entry is the open one', () => {
    expect(followEntryDelete('e1', event, '2026-10-24')).toEqual({
      message: '‘Wed 21 Oct’ was deleted on Pixel 8',
    });
    expect(followEntryDelete('e2', event, '2026-10-24')).toBeNull();
    expect(followEntryDelete(null, event, '2026-10-24')).toBeNull();
  });
});

describe('Restore after a remote delete', () => {
  const run = async (stored: string, saved: string, text: string) => {
    const update = vi.fn(async () => undefined);
    const restore = vi.fn(async () => ({ body: stored }));
    await restoreWithDraft({ restore, update, session: () => 's' }, 'e1', { text, saved });
    return { restore, update };
  };

  it('brings the Entry back with the text that was being typed, once', async () => {
    const { restore, update } = await run('first draft', 'first draft', 'first draft, typed on');
    expect(restore).toHaveBeenCalledWith('e1');
    expect(update).toHaveBeenCalledTimes(1);
    expect(update).toHaveBeenCalledWith('e1', 'first draft, typed on', 's');
  });

  it('writes nothing more when the typed text was already saved or is blank', async () => {
    expect((await run('same', 'same', 'same')).update).not.toHaveBeenCalled();
    expect((await run('same', 'same', '  \n')).update).not.toHaveBeenCalled();
  });

  it('keeps both when the Entry was also edited elsewhere', async () => {
    const added = await run(
      'first draft\nline from the phone',
      'first draft',
      'first draft, typed on',
    );
    expect(added.update).toHaveBeenCalledWith(
      'e1',
      'first draft\nline from the phone, typed on',
      's',
    );
    const rewritten = await run('phone text', 'first draft', 'rewritten here');
    expect(rewritten.update).toHaveBeenCalledWith('e1', 'phone text\n\nrewritten here', 's');
    // Nothing typed since the last save: the restored text stands.
    expect((await run('phone text', 'first draft', 'first draft')).update).not.toHaveBeenCalled();
  });
});
