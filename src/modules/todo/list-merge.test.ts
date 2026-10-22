import { describe, expect, it } from 'vitest';
import type { TodoListsMergedEvent } from '../../lib/ipc/bindings';
import { followMerge, mergedMessage } from './list-merge';
import { listView, smartView } from './views';

const event: TodoListsMergedEvent = {
  from_id: 'new',
  from_name: 'Groceries',
  into_id: 'old',
  into_name: 'Groceries',
  device: 'Pixel 8',
};

describe('list merge', () => {
  it('says which List merged with which, and from where', () => {
    expect(mergedMessage(event)).toBe('Groceries merged with Groceries from Pixel 8');
    expect(mergedMessage({ ...event, device: null })).toBe('Groceries merged with Groceries');
  });

  it('moves someone on the List that went away to the one that was kept', () => {
    expect(followMerge(listView('new'), event)).toEqual({
      view: listView('old'),
      message: 'Groceries merged with Groceries from Pixel 8',
    });
  });

  it('leaves everyone else where they are', () => {
    expect(followMerge(listView('old'), event)).toBeNull();
    expect(followMerge(listView('other'), event)).toBeNull();
    expect(followMerge(smartView('inbox'), event)).toBeNull();
  });
});
