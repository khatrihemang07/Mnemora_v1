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

describe('a change that waited for the editor (M8, Rust held.rs)', () => {
  const entry = (text: string) => ({
    page_date: '2026-10-20',
    text,
    written_tz: 'UTC',
    written_offset_min: 0,
    session_id: 's',
  });
  const setupHeld = () => {
    const mock = createJournalMock();
    const made = mock.handle('journal_entry_create', {
      entry: entry('one\ntwo\nthree\n'),
    }) as Entry;
    const hold = { entity: 'journal_entry', entityId: made.id, scope: '*' };
    const body = () => {
      const pages = mock.handle('journal_pages', { cursor: null, limit: 5, direction: 'older' });
      return (pages as { entries: Entry[] }[])[0]?.entries[0]?.body;
    };
    const type = (text: string) =>
      mock.handle('journal_entry_update', { id: made.id, text, sessionId: 's' });
    mock.handle('editing_hold', hold);
    return { mock, id: made.id, hold, body, type };
  };

  it('keeps what was typed beside the other device’s change when they touch different lines', () => {
    const { mock, id, hold, body, type } = setupHeld();
    mock.simulateRemoteEdit('one\ntwo\nthree, from elsewhere\n', id);
    type('one, typed\ntwo\nthree\n');
    mock.handle('editing_release', hold);
    expect(body()).toBe('one, typed\ntwo\nthree, from elsewhere\n');
  });

  it('on a clash the typing shows, and the other text goes to a Conflict', () => {
    const { mock, id, hold, body, type } = setupHeld();
    const clash = vi.fn();
    mock.onHeldClash = clash;
    mock.simulateRemoteEdit('one\ntwo, remote\nthree\n', id);
    type('one\ntwo, local\nthree\n');
    mock.handle('editing_release', hold);
    expect(body()).toBe('one\ntwo, local\nthree\n');
    expect(clash).toHaveBeenCalledWith(
      expect.objectContaining({
        base: 'one\ntwo\nthree\n',
        showing: 'one\ntwo, local\nthree\n',
        other: 'one\ntwo, remote\nthree\n',
      }),
    );
  });

  it('takes the other text whole when nothing was typed', () => {
    const { mock, id, hold, body } = setupHeld();
    mock.simulateRemoteEdit('elsewhere', id);
    mock.handle('editing_release', hold);
    expect(body()).toBe('elsewhere');
  });
});
