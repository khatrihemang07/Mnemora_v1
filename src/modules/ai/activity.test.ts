import { describe, expect, it } from 'vitest';
import type { EntryView, Source } from '../../lib/ipc/bindings';
import {
  completedLabel,
  openHref,
  runningLabel,
  sourceModule,
  stepsByQuestion,
  withoutCitations,
} from './activity';

const ID = '01990000-0000-7000-8000-000000000001';
const journal: Source = { id: ID, module: 'journal', title: 'Long walk', date: '2026-10-22' };
const task: Source = { id: ID, module: 'todo', title: 'Book the dentist', date: '2026-10-27' };

describe('activity labels', () => {
  it('say what is happening and count the steps in plain words', () => {
    expect(runningLabel(0)).toBe('Reading your journal and tasks…');
    expect(runningLabel(1)).toBe('Reading your journal and tasks… · 1 step done');
    expect(runningLabel(2)).toBe('Reading your journal and tasks… · 2 steps done');
    expect(completedLabel(1)).toBe('Completed 1 step');
    expect(completedLabel(3)).toBe('Completed 3 steps');
  });
});

describe('sources', () => {
  it('opens a Journal item on its Page and a Task in the Todo sheet', () => {
    expect(openHref(journal)).toBe('/journal/2026-10-22');
    expect(openHref(task)).toBe(`/todo?task=${ID}`);
    expect(sourceModule(journal)).toBe('Journal');
    expect(sourceModule(task)).toBe('Todo');
  });

  it('hides the id markers the Model wrote, and leaves other brackets alone', () => {
    expect(withoutCitations(`You walked [${ID}] and rested.`)).toBe('You walked and rested.');
    expect(withoutCitations('See [today] only.')).toBe('See [today] only.');
  });
});

describe('stepsByQuestion', () => {
  const entry = (id: string, payload: EntryView['payload']): EntryView => ({
    id,
    parent_id: null,
    created_at: 0,
    payload,
  });

  it('gives each question the tool results saved after it and before the next question', () => {
    const entries: EntryView[] = [
      entry('q1', { kind: 'user', text: 'First' }),
      entry('t1', {
        kind: 'tool_result',
        call_id: 'c1',
        name: 'journal_list',
        arguments: '{}',
        result: 'r',
        is_error: false,
        observation_id: 'e1',
        summary: 'Listed entries, 7 entries',
      }),
      entry('a1', {
        kind: 'assistant',
        text: 'Done',
        stop: 'stop',
        provider: 'P',
        model: 'm',
        calls: [],
        sources: [],
        budget_note: null,
      }),
      entry('q2', { kind: 'user', text: 'Second' }),
    ];
    const steps = stepsByQuestion(entries);
    expect(steps.get('q1')).toEqual([
      { name: 'journal_list', summary: 'Listed entries, 7 entries', isError: false },
    ]);
    expect(steps.get('q2')).toEqual([]);
  });
});
