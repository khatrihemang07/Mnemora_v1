import { describe, expect, it, vi } from 'vitest';
import type { Entry } from './bindings';
import { createJournalMock } from './mock-journal';

function setup() {
  const mock = createJournalMock();
  const made = mock.handle('journal_entry_create', {
    entry: {
      page_date: '2026-10-20',
      text: 'first draft',
      written_tz: 'UTC',
      written_offset_min: 0,
      session_id: 's',
    },
  }) as Entry;
  const body = () => {
    const pages = mock.handle('journal_pages', { cursor: null, limit: 5, direction: 'older' });
    return (pages as { entries: Entry[] }[])[0]?.entries[0]?.body;
  };
  return { mock, id: made.id, body };
}

describe('the simulated other device in the Journal mock', () => {
  it('edits an Entry at once when nobody is editing it', () => {
    const { mock, id, body } = setup();
    expect(mock.simulateRemoteEdit('from elsewhere', id)).toBe(true);
    expect(body()).toBe('from elsewhere');
  });

  it('waits for an Entry that is held and applies it when the editor lets go', () => {
    const { mock, id, body } = setup();
    const changed = vi.fn();
    mock.onRemoteChange = changed;
    mock.handle('editing_hold', { entity: 'journal_entry', entityId: id, scope: '*' });
    expect(mock.simulateRemoteEdit('from elsewhere', id)).toBe(false);
    expect(body()).toBe('first draft');
    mock.handle('editing_release', { entity: 'journal_entry', entityId: id, scope: '*' });
    expect(body()).toBe('from elsewhere');
    expect(changed).toHaveBeenCalledOnce();
  });
});
