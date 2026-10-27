import { describe, expect, it } from 'vitest';
import type { EntryView } from '../../lib/ipc/bindings';
import { questionsByAnswer } from './answers';

const entry = (id: string, payload: EntryView['payload']): EntryView => ({
  id,
  parent_id: null,
  created_at: 0,
  payload,
});
const answer = (id: string) =>
  entry(id, {
    kind: 'assistant',
    text: 'x',
    stop: 'stop',
    provider: 'P',
    model: 'm',
    calls: [],
    sources: [],
    budget_note: null,
  });

describe('questionsByAnswer', () => {
  it('pairs each answer with the question written before it', () => {
    const map = questionsByAnswer([
      entry('q1', { kind: 'user', text: 'First' }),
      answer('a1'),
      entry('q2', { kind: 'user', text: 'Second' }),
      answer('a2'),
    ]);
    expect(map.get('a1')).toBe('First');
    expect(map.get('a2')).toBe('Second');
  });

  it('leaves out an answer that has no question before it', () => {
    expect(questionsByAnswer([answer('a0')]).has('a0')).toBe(false);
  });
});
