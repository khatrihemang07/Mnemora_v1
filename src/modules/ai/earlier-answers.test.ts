import { describe, expect, it } from 'vitest';
import type { EntryView } from '../../lib/ipc/bindings';
import { ANSWER_START_CHARS, earlierAnswers, excerpt } from './earlier-answers';

const entry = (id: string, payload: EntryView['payload']): EntryView => ({
  id,
  parent_id: null,
  created_at: 0,
  payload,
  sibling_count: 1,
  sibling_index: 1,
});
const question = (id: string, text: string) => entry(id, { kind: 'user', text });
const answer = (id: string, text: string, stop: 'stop' | 'aborted' = 'stop') =>
  entry(id, {
    kind: 'assistant',
    text,
    stop,
    provider: 'P',
    model: 'm',
    calls: [],
    sources: [],
    budget_note: null,
  });
const step = (id: string) =>
  entry(id, {
    kind: 'assistant',
    text: '',
    stop: 'stop',
    provider: 'P',
    model: 'm',
    calls: [{ id: 'c1', name: 'journal_list', arguments: '{}' }],
    sources: [],
    budget_note: null,
  });

describe('earlierAnswers', () => {
  it('lists each answer with the question it answers, oldest first', () => {
    expect(
      earlierAnswers([
        question('q1', 'First question'),
        answer('a1', 'Answer one'),
        question('q2', 'Second question'),
        answer('a2', 'Answer two'),
      ]),
    ).toEqual([
      { answerId: 'a1', question: 'First question', start: 'Answer one', stopped: false },
      { answerId: 'a2', question: 'Second question', start: 'Answer two', stopped: false },
    ]);
  });

  it('skips a Step that asks for Tools, which is not an answer', () => {
    const rows = earlierAnswers([question('q1', 'Find it'), step('s1'), answer('a1', 'Found it')]);
    expect(rows.map((row) => row.answerId)).toEqual(['a1']);
  });

  it('includes a stopped answer as it stands, marked stopped', () => {
    expect(
      earlierAnswers([question('q1', 'Long one'), answer('a1', 'Partly written', 'aborted')]),
    ).toEqual([{ answerId: 'a1', question: 'Long one', start: 'Partly written', stopped: true }]);
  });

  it('leaves out an answer that has no question before it', () => {
    expect(earlierAnswers([answer('a0', 'Orphan')])).toEqual([]);
  });
});

describe('excerpt', () => {
  it('collapses whitespace and drops citation markers', () => {
    expect(excerpt('Walks  in the park\n[01990000-0000-7000-8000-0000000000e1] today.')).toBe(
      'Walks in the park today.',
    );
  });

  it('cuts a long text to the start length, with an ellipsis', () => {
    const cut = excerpt('a'.repeat(ANSWER_START_CHARS + 20));
    expect(Array.from(cut)).toHaveLength(ANSWER_START_CHARS + 1);
    expect(cut.endsWith('…')).toBe(true);
  });
});
