import { describe, expect, it } from 'vitest';
import type { TodoList } from '../../lib/ipc/bindings';
import { deletedListMessage, movedMessage } from './lists';

const home = { id: 'l1', name: 'Home' } as TodoList;

describe('deletedListMessage', () => {
  it('says only the List went when it was empty, whatever the choice', () => {
    expect(deletedListMessage(home, 0, { kind: 'delete_tasks' })).toBe('Deleted “Home”');
    expect(deletedListMessage(home, 0, { kind: 'move_to_inbox' })).toBe('Deleted “Home”');
  });

  it('says what became of the Tasks', () => {
    expect(deletedListMessage(home, 3, { kind: 'delete_tasks' })).toBe(
      'Deleted “Home” and 3 Tasks',
    );
    expect(deletedListMessage(home, 1, { kind: 'delete_tasks' })).toBe('Deleted “Home” and 1 Task');
    expect(deletedListMessage(home, 2, { kind: 'move_to_inbox' })).toBe(
      'Deleted “Home”; 2 Tasks moved to the Inbox',
    );
    expect(deletedListMessage(home, 2, { kind: 'move_to_list', list_id: 'l2' }, 'Work')).toBe(
      'Deleted “Home”; 2 Tasks moved to “Work”',
    );
  });
});

describe('movedMessage', () => {
  it('names the List or the Inbox', () => {
    expect(movedMessage(home)).toBe('Moved to “Home”');
    expect(movedMessage(null)).toBe('Moved to the Inbox');
  });
});
