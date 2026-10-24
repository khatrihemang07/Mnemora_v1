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
  const run = async (stored: string, draft: string) => {
    const update = vi.fn(async () => undefined);
    const restore = vi.fn(async () => ({ body: stored }));
    await restoreWithDraft({ restore, update, session: () => 's' }, 'e1', draft);
    return { restore, update };
  };

  it('brings the Entry back with the text that was being typed', async () => {
    const { restore, update } = await run('first draft', 'first draft, typed on');
    expect(restore).toHaveBeenCalledWith('e1');
    expect(update).toHaveBeenCalledWith('e1', 'first draft, typed on', 's');
  });

  it('writes nothing more when the typed text was already saved or is blank', async () => {
    expect((await run('same', 'same')).update).not.toHaveBeenCalled();
    expect((await run('same', '  \n')).update).not.toHaveBeenCalled();
  });
});
