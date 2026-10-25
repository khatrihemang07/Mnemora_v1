import { describe, expect, it } from 'vitest';
import type { AiRunEvent, RunEvent } from '../../lib/ipc/bindings';
import { answeringIn, applyRunEvent, failedIn, type LiveRuns } from './run-state';

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
      { runId: 'r1', conversationId: 'c1', text: 'Hello', failure: null },
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

  it('ignores entries being saved, which the page reads from the Conversation', () => {
    const runs = fold([step({ type: 'runStarted' })]);
    expect(applyRunEvent(runs, step({ type: 'entrySaved', entry_id: 'e1', kind: 'user' }))).toBe(
      runs,
    );
  });
});
