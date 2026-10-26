import { describe, expect, it } from 'vitest';
import type { EntryView } from '../../lib/ipc/bindings';
import { questionOfAnswers, questionsByAnswer } from './answers';

const entry = (id: string, payload: EntryView['payload']): EntryView => ({
  id,
  parent_id: null,
  created_at: 0,
  payload,
  sibling_count: 1,
  sibling_index: 1,
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

describe('questionOfAnswers', () => {
  it('gives each answer the id and the text of its question, for Try again and Edit', () => {
    const map = questionOfAnswers([
      entry('q1', { kind: 'user', text: 'First' }),
      answer('a1'),
      entry('q2', { kind: 'user', text: 'Second' }),
      answer('a2'),
    ]);
    expect(map.get('a1')).toEqual({ id: 'q1', text: 'First' });
    expect(map.get('a2')).toEqual({ id: 'q2', text: 'Second' });
  });
});
