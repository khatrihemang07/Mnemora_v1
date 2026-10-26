import type { AiRunEvent, EntryView, Failure } from '../../lib/ipc/bindings';

/** Why a Run failed: the kind picks the banner, the message is the one sentence shown. */
export interface RunFailure {
  readonly kind: Failure;
  readonly message: string;
}

/** One Tool call of a Run: running (no summary yet) or finished with its plain line. */
export interface LiveStep {
  readonly callId: string;
  readonly name: string;
  /** The plain line the activity shows; null while the call runs. */
  readonly summary: string | null;
  readonly isError: boolean;
}

/** A Run this page has seen since it opened, and has not seen answered. */
export interface LiveRun {
  readonly runId: string;
  readonly conversationId: string;
  /** The entry id of the question this Run answers, once it is saved. Places the Run in the thread. */
  readonly questionId: string | null;
  /** The answer as streamed so far (the current Step's text). */
  readonly text: string;
  /** The Tool calls made so far in this Run, in order. */
  readonly steps: readonly LiveStep[];
  /** Set when the Run failed: shown in the answer's place. */
  readonly failure: RunFailure | null;
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
    questionId: null,
    text: '',
    steps: [],
    failure: null,
  };
  const step = event.event;
  switch (step.type) {
    case 'queueChanged':
      // The notes waiting are kept by the Conversation, not by the Run (see queue-state).
      return runs;
    case 'runStarted':
      // A new Run in this Conversation makes its earlier failures stale, so their banners go.
      return { ...withoutFailedIn(runs, conversationId), [runId]: current };
    case 'textDelta':
      return { ...runs, [runId]: { ...current, text: current.text + step.text } };
    case 'stepStarted':
      // Each Step's text starts afresh: a Step that asks for Tools writes little or none.
      return { ...runs, [runId]: { ...current, text: '' } };
    case 'toolStarted':
      return {
        ...runs,
        [runId]: {
          ...current,
          steps: [
            ...current.steps,
            { callId: step.call_id, name: step.name, summary: null, isError: false },
          ],
        },
      };
    case 'toolEnded':
      return {
        ...runs,
        [runId]: {
          ...current,
          steps: current.steps.map((known) =>
            known.callId === step.call_id
              ? { ...known, summary: step.summary, isError: step.is_error }
              : known,
          ),
        },
      };
    case 'runEnded': {
      // A stopped Run is saved as it stood, marked stopped, so the thread shows it from the Conversation.
      if (step.outcome.type === 'answered' || step.outcome.type === 'stopped') {
        return withoutRun(runs, runId);
      }
      return {
        ...runs,
        [runId]: {
          ...current,
          failure: { kind: step.outcome.kind, message: step.outcome.message },
        },
      };
    }
    case 'entrySaved':
      if (step.kind !== 'user') return runs;
      return { ...runs, [runId]: { ...current, questionId: step.entry_id } };
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

/**
 * The failed Runs whose banner is still shown: only one answering the last question of the Branch
 * shown. Once the Branch moves on (another question, or an earlier Branch), the banner is gone.
 */
export function bannerRuns(failed: LiveRun[], lastQuestion: string | null): LiveRun[] {
  if (lastQuestion === null) return [];
  return failed.filter((run) => run.questionId === lastQuestion);
}

/** The newest question on the Branch shown, or null when it has none. */
export function lastQuestionId(entries: readonly EntryView[]): string | null {
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    const entry = entries[index];
    if (entry?.payload.kind === 'user') return entry.id;
  }
  return null;
}

/** The Runs (answering or failed) that answer one question of the thread. */
export function runsUnder(runs: LiveRun[], questionId: string): LiveRun[] {
  return runs.filter((run) => run.questionId === questionId);
}

/** The Runs whose question is not among the thread's entries (not saved yet, or not loaded). */
export function unplacedRuns(runs: LiveRun[], entryIds: ReadonlySet<string>): LiveRun[] {
  return runs.filter((run) => run.questionId === null || !entryIds.has(run.questionId));
}

function withoutFailedIn(runs: LiveRuns, conversationId: string): LiveRuns {
  return Object.fromEntries(
    Object.entries(runs).filter(
      ([, run]) => run.conversationId !== conversationId || run.failure === null,
    ),
  );
}

function withoutRun(runs: LiveRuns, runId: string): LiveRuns {
  return Object.fromEntries(Object.entries(runs).filter(([id]) => id !== runId));
}
