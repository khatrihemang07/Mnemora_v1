import { describe, expect, it } from 'vitest';
import type { Entry, Page, Source, Task } from '../../lib/ipc/bindings';
import { createJournalMock } from '../../lib/ipc/mock-journal';
import { createTodoMock } from '../../lib/ipc/mock-todo';
import { firstLines, loadSource, type SourceReader } from './source-excerpt';

const journalSource: Source = { id: 'e1', module: 'journal', title: 'Museum', date: '2026-10-22' };
const todoSource: Source = { id: 't1', module: 'todo', title: 'Book dentist', date: '2026-10-30' };

const entry = (id: string, body: string): Entry => ({ id, page_date: '2026-10-22', body }) as Entry;

const page = (entries: Entry[]): Page => ({
  date: '2026-10-22',
  entry_count: entries.length,
  word_count: 0,
  entries,
});

function readerWith(parts: Partial<SourceReader>): SourceReader {
  return {
    journalPage: async () => [],
    todoTask: async () => {
      throw new Error('unused');
    },
    ...parts,
  };
}

describe('firstLines', () => {
  it('keeps the first three non-empty lines and drops the blank ones', () => {
    expect(firstLines('One\n\nTwo\n  \nThree\nFour')).toBe('One\nTwo\nThree');
  });

  it('cuts a long excerpt at 240 characters with an ellipsis', () => {
    const long = 'word '.repeat(100);
    const cut = firstLines(long);
    expect(cut.endsWith('…')).toBe(true);
    expect(cut.length).toBeLessThanOrEqual(241);
  });
});

describe('loadSource', () => {
  it('reads the Entry on its Page and gives its first lines', async () => {
    const read = readerWith({
      journalPage: async (date) => {
        expect(date).toBe('2026-10-22');
        return [
          page([entry('other', 'Not it'), entry('e1', 'The museum opens at ten.\nBring cash.')]),
        ];
      },
    });
    await expect(loadSource(journalSource, read)).resolves.toEqual({
      title: 'Museum',
      date: '2026-10-22',
      excerpt: 'The museum opens at ten.\nBring cash.',
    });
  });

  it('says an Entry that is gone no longer exists', async () => {
    const read = readerWith({ journalPage: async () => [page([entry('other', 'x')])] });
    await expect(loadSource(journalSource, read)).rejects.toThrow('That Entry no longer exists');
  });

  it('says so when the Source has no Page date to read from', async () => {
    const read = readerWith({});
    await expect(loadSource({ ...journalSource, date: null }, read)).rejects.toThrow(
      'That Entry no longer exists',
    );
  });

  it('reads a Task through the Todo read and uses its notes', async () => {
    const read = readerWith({
      todoTask: async (id) => {
        expect(id).toBe('t1');
        return { title: 'Book dentist', notes: 'Ask for Tuesday.', due_date: '2026-10-30' } as Task;
      },
    });
    await expect(loadSource(todoSource, read)).resolves.toEqual({
      title: 'Book dentist',
      date: '2026-10-30',
      excerpt: 'Ask for Tuesday.',
    });
  });

  it('lets a Task that is gone report its own error', async () => {
    const read = readerWith({
      todoTask: async () => {
        throw new Error('That Task no longer exists');
      },
    });
    await expect(loadSource(todoSource, read)).rejects.toThrow('That Task no longer exists');
  });
});

describe('loadSource against the browser mock', () => {
  it('reads a written Entry and a Task, and says a deleted Task no longer exists', async () => {
    const journal = createJournalMock();
    const todo = createTodoMock();
    const made = journal.handle('journal_entry_create', {
      entry: {
        page_date: '2026-10-22',
        text: 'The museum opens at ten.\nBring cash.',
        written_tz: 'UTC',
        written_offset_min: 0,
        session_id: 's',
      },
    }) as Entry;
    const created = todo.handle('todo_task_create', { title: 'Book dentist' }) as Task;
    const task = todo.handle('todo_task_update', {
      id: created.id,
      title: 'Book dentist',
      notes: 'Ask for Tuesday.',
      sessionId: 's',
    }) as Task;
    const read: SourceReader = {
      journalPage: async (date) =>
        journal.handle('journal_pages', { cursor: date, limit: 1, direction: 'from' }) as Page[],
      todoTask: async (id) => todo.handle('todo_task', { id }) as Task,
    };

    await expect(
      loadSource({ id: made.id, module: 'journal', title: 'Museum', date: '2026-10-22' }, read),
    ).resolves.toMatchObject({ excerpt: 'The museum opens at ten.\nBring cash.' });
    await expect(
      loadSource({ id: task.id, module: 'todo', title: 'Book dentist', date: null }, read),
    ).resolves.toMatchObject({ excerpt: 'Ask for Tuesday.' });

    todo.handle('todo_task_delete', { id: task.id });
    await expect(
      loadSource({ id: task.id, module: 'todo', title: 'Book dentist', date: null }, read),
    ).rejects.toThrow('That Task no longer exists');
  });
});
