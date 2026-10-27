import type { AiRunEvent } from '../../lib/ipc/bindings';

/** A Run this page has seen since it opened, and has not seen answered. */
export interface LiveRun {
  readonly runId: string;
  readonly conversationId: string;
  /** The answer as streamed so far. */
  readonly text: string;
  /** Set when the Run failed: the one sentence to show in the answer's place. */
  readonly failure: string | null;
}

/** Live Runs by Run id. */
export type LiveRuns = Readonly<Record<string, LiveRun>>;

/**
 * Folds one run event into the live Runs. An answered Run is dropped: its answer is saved in the
 * Conversation by then, and the page reads it from there. A failed Run stays, with its sentence,
 * until the page opens again.
 */
export function applyRunEvent(runs: LiveRuns, event: AiRunEvent): LiveRuns {
  const { run_id: runId, conversation_id: conversationId } = event;
  const current: LiveRun = runs[runId] ?? {
    runId,
    conversationId,
    text: '',
    failure: null,
  };
  const step = event.event;
  switch (step.type) {
    case 'runStarted':
      return { ...runs, [runId]: current };
    case 'textDelta':
      return { ...runs, [runId]: { ...current, text: current.text + step.text } };
    case 'runEnded': {
      if (step.outcome.type === 'answered') return withoutRun(runs, runId);
      return { ...runs, [runId]: { ...current, failure: step.outcome.message } };
    }
    case 'entrySaved':
    case 'stepStarted':
      return runs;
  }
}

/** The Runs in one Conversation that are still answering (not failed). */
export function answeringIn(runs: LiveRuns, conversationId: string): LiveRun[] {
  return Object.values(runs).filter(
    (run) => run.conversationId === conversationId && run.failure === null,
  );
}

/** The failed Runs in one Conversation, with their sentences. */
export function failedIn(runs: LiveRuns, conversationId: string): LiveRun[] {
  return Object.values(runs).filter(
    (run) => run.conversationId === conversationId && run.failure !== null,
  );
}

function withoutRun(runs: LiveRuns, runId: string): LiveRuns {
  return Object.fromEntries(Object.entries(runs).filter(([id]) => id !== runId));
}
