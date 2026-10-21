import { describe, expect, it } from 'vitest';
import { readTodo } from './settings';

describe('readTodo', () => {
  it('fills the defaults when nothing is saved', () => {
    expect(readTodo()).toEqual({ parseDates: true, doneInLists: 'today_shown' });
  });

  it('reads saved values', () => {
    expect(readTodo({ 'todo.parse_dates': false, 'todo.done_in_lists': 'hidden' })).toEqual({
      parseDates: false,
      doneInLists: 'hidden',
    });
  });

  it('falls back on a value it does not know', () => {
    expect(
      readTodo({ 'todo.done_in_lists': 'sideways', 'todo.parse_dates': 'x' }).doneInLists,
    ).toBe('today_shown');
  });
});
