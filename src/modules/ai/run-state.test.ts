import { describe, expect, it } from 'vitest';
import type { AiRunEvent, EntryView, RunEvent } from '../../lib/ipc/bindings';
import {
  answeringIn,
  applyRunEvent,
  bannerRuns,
  failedIn,
  type LiveRuns,
  lastQuestionId,
  runsUnder,
  unplacedRuns,
} from './run-state';

function step(event: RunEvent, run = 'r1', conversation = 'c1'): AiRunEvent {
  return { run_id: run, conversation_id: conversation, event };
}

function fold(events: AiRunEvent[]): LiveRuns {
  return events.reduce(applyRunEvent, {} as LiveRuns);
}

describe('applyRunEvent', () => {
  it('streams the text in order and drops the Run once it is answered', () => {
    const mid = fold([
      step({ type: 'runStarted' }),
      step({ type: 'stepStarted' }),
      step({ type: 'textDelta', text: 'Hel' }),
      step({ type: 'textDelta', text: 'lo' }),
    ]);
    expect(answeringIn(mid, 'c1')).toEqual([
      {
        runId: 'r1',
        conversationId: 'c1',
        questionId: null,
        text: 'Hello',
        steps: [],
        failure: null,
      },
    ]);

    const done = applyRunEvent(mid, step({ type: 'runEnded', outcome: { type: 'answered' } }));
    expect(done).toEqual({});
  });

  it('keeps a failed Run with its sentence, and it is no longer answering', () => {
    const runs = fold([
      step({ type: 'runStarted' }),
      step({
        type: 'runEnded',
        outcome: { type: 'failed', kind: 'unreachable', message: 'Could not reach it.' },
      }),
    ]);
    expect(answeringIn(runs, 'c1')).toEqual([]);
    expect(failedIn(runs, 'c1')).toEqual([
      {
        runId: 'r1',
        conversationId: 'c1',
        questionId: null,
        steps: [],
        text: '',
        failure: { kind: 'unreachable', message: 'Could not reach it.' },
      },
    ]);
  });

  it('keeps the Runs of different Conversations apart', () => {
    const runs = fold([
      step({ type: 'runStarted' }, 'r1', 'c1'),
      step({ type: 'runStarted' }, 'r2', 'c2'),
      step({ type: 'textDelta', text: 'other' }, 'r2', 'c2'),
    ]);
    expect(answeringIn(runs, 'c1').map((r) => r.runId)).toEqual(['r1']);
    expect(answeringIn(runs, 'c2')[0]?.text).toBe('other');
  });

  it('ignores saved answers, which the page reads from the Conversation', () => {
    const runs = fold([step({ type: 'runStarted' })]);
    expect(
      applyRunEvent(runs, step({ type: 'entrySaved', entry_id: 'e1', kind: 'assistant' })),
    ).toBe(runs);
  });

  it('remembers the question a Run answers, from the question entry saved for it', () => {
    const runs = fold([
      step({ type: 'runStarted' }),
      step({ type: 'entrySaved', entry_id: 'q1', kind: 'user' }),
      step({ type: 'stepStarted' }),
    ]);
    expect(runs.r1?.questionId).toBe('q1');
  });

  it('keeps a failed Run under the question it answers, not with the others', () => {
    const runs = fold([
      step({ type: 'runStarted' }),
      step({ type: 'entrySaved', entry_id: 'q1', kind: 'user' }),
      step({
        type: 'runEnded',
        outcome: { type: 'failed', kind: 'timeout', message: 'Took too long.' },
      }),
    ]);
    expect(runsUnder(failedIn(runs, 'c1'), 'q1').map((r) => r.runId)).toEqual(['r1']);
    expect(runsUnder(failedIn(runs, 'c1'), 'q2')).toEqual([]);
  });
});

describe('applyRunEvent with Tool steps', () => {
  it('lists each Tool call while it runs and gives it its summary when it ends', () => {
    const runs = [
      step({ type: 'runStarted' }),
      step({ type: 'stepStarted' }),
      step({ type: 'toolStarted', call_id: 'c1', name: 'journal_list' }),
      step({
        type: 'toolEnded',
        call_id: 'c1',
        name: 'journal_list',
        summary: 'Listed entries, 7 entries',
        is_error: false,
      }),
      step({ type: 'stepStarted' }),
      step({ type: 'toolStarted', call_id: 'c2', name: 'todo_list' }),
    ].reduce<LiveRuns>(applyRunEvent, {});
    expect(answeringIn(runs, 'c1')[0]?.steps).toEqual([
      { callId: 'c1', name: 'journal_list', summary: 'Listed entries, 7 entries', isError: false },
      { callId: 'c2', name: 'todo_list', summary: null, isError: false },
    ]);
  });

  it('clears the text of a Step when the next Step starts, keeping the steps', () => {
    const runs = [
      step({ type: 'runStarted' }),
      step({ type: 'textDelta', text: 'Let me look.' }),
      step({ type: 'toolStarted', call_id: 'c1', name: 'journal_list' }),
      step({ type: 'stepStarted' }),
    ].reduce<LiveRuns>(applyRunEvent, {});
    const run = answeringIn(runs, 'c1')[0];
    expect(run?.text).toBe('');
    expect(run?.steps).toHaveLength(1);
  });
});

describe('unplacedRuns', () => {
  it('returns the Runs whose question is not in the thread, so they go at the end', () => {
    const runs = fold([
      step({ type: 'runStarted' }, 'r1'),
      step({ type: 'entrySaved', entry_id: 'q1', kind: 'user' }, 'r1'),
      step({ type: 'runStarted' }, 'r2'),
    ]);
    const live = answeringIn(runs, 'c1');
    expect(unplacedRuns(live, new Set(['q1'])).map((r) => r.runId)).toEqual(['r2']);
    expect(unplacedRuns(live, new Set(['other'])).map((r) => r.runId)).toEqual(['r1', 'r2']);
  });
});

describe('stopping and the queue', () => {
  it('a stopped Run leaves the live list, since its saved part shows in the thread', () => {
    const runs = fold([
      step({ type: 'runStarted' }),
      step({ type: 'textDelta', text: 'Part' }),
      step({ type: 'runEnded', outcome: { type: 'stopped' } }),
    ]);
    expect(runs).toEqual({});
  });

  it('a queue change is not a Run of its own', () => {
    const runs = fold([step({ type: 'queueChanged', notes: [{ id: 'n1', text: 'later' }] })]);
    expect(runs).toEqual({});
  });
});

function failedRun(run: string, question: string | null, conversation = 'c1'): AiRunEvent[] {
  return [
    step({ type: 'runStarted' }, run, conversation),
    ...(question === null
      ? []
      : [step({ type: 'entrySaved', entry_id: question, kind: 'user' }, run, conversation)]),
    step(
      {
        type: 'runEnded',
        outcome: { type: 'failed', kind: 'too_long', message: 'Too long.' },
      },
      run,
      conversation,
    ),
  ];
}

describe('the banner of a failed Run', () => {
  it("a new Run in a Conversation drops that Conversation's failed Runs, and no other", () => {
    const runs = fold([
      ...failedRun('r1', 'q1', 'c1'),
      ...failedRun('r2', 'q9', 'c2'),
      step({ type: 'runStarted' }, 'r3', 'c1'),
    ]);
    expect(failedIn(runs, 'c1')).toEqual([]);
    expect(failedIn(runs, 'c2').map((run) => run.runId)).toEqual(['r2']);
  });

  it('is shown only while its question is the last question of the Branch', () => {
    const failed = failedIn(fold(failedRun('r1', 'q1')), 'c1');
    expect(bannerRuns(failed, 'q1').map((run) => run.runId)).toEqual(['r1']);
    expect(bannerRuns(failed, 'q2')).toEqual([]);
    expect(bannerRuns(failed, null)).toEqual([]);
  });
});

describe('lastQuestionId', () => {
  const entry = (id: string, kind: 'user' | 'assistant'): EntryView =>
    ({
      id,
      parent_id: null,
      created_at: 0,
      payload:
        kind === 'user'
          ? { kind, text: id }
          : {
              kind,
              text: id,
              stop: 'stop',
              provider: 'P',
              model: 'm',
              calls: [],
              sources: [],
              budget_note: null,
            },
      sibling_count: 1,
      sibling_index: 1,
    }) as EntryView;

  it('is the newest question on the Branch, even when answers or Steps follow it', () => {
    expect(
      lastQuestionId([
        entry('q1', 'user'),
        entry('a1', 'assistant'),
        entry('q2', 'user'),
        entry('a2', 'assistant'),
      ]),
    ).toBe('q2');
  });

  it('is null when the Branch has no question', () => {
    expect(lastQuestionId([])).toBeNull();
  });
});
