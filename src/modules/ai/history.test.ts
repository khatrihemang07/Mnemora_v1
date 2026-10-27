import { describe, expect, it } from 'vitest';
import type { ConversationSummary } from '../../lib/ipc/bindings';
import { groupConversations, groupOf } from './history';

// Local noon keeps the instants clear of any clock change near midnight.
const at = (day: number) => new Date(2026, 9, day, 12).getTime();
const now = new Date(2026, 9, 25, 9);

describe('groupOf', () => {
  it('puts a change today, yesterday, in the previous 7 days or earlier', () => {
    expect(groupOf(at(25), now)).toBe('today');
    expect(groupOf(at(24), now)).toBe('yesterday');
    expect(groupOf(at(23), now)).toBe('previous7');
    expect(groupOf(at(18), now)).toBe('previous7');
    expect(groupOf(at(17), now)).toBe('earlier');
  });

  it('counts a time ahead of now as today', () => {
    expect(groupOf(at(26), now)).toBe('today');
  });
});

describe('groupConversations', () => {
  const item = (id: string, day: number): ConversationSummary => ({
    id,
    title: id,
    created_at: at(day),
    updated_at: at(day),
  });

  it('keeps the newest-first order inside each group and drops empty groups', () => {
    const sections = groupConversations(
      [item('a', 25), item('b', 24), item('c', 20), item('d', 25)],
      now,
    );
    expect(sections.map((s) => [s.label, s.items.map((i) => i.id)])).toEqual([
      ['Today', ['a', 'd']],
      ['Yesterday', ['b']],
      ['Previous 7 days', ['c']],
    ]);
  });
});
