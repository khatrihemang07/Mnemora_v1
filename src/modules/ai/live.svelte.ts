import type { AiRunEvent } from '../../lib/ipc/bindings';
import type { Choice } from './choice';
import { applyRunEvent, type LiveRuns } from './run-state';

/** What a question was sent with, kept so "Try again" can send it again. */
export interface SentQuestion extends Choice {
  readonly text: string;
  readonly conversationId: string;
}

/**
 * The Runs the open AI page has seen. In memory only: a reload forgets them, and the saved
 * Conversation is the record.
 */
class LiveRunStore {
  runs = $state<LiveRuns>({});
  /** The questions this page sent, by Run id. */
  sent = $state<Readonly<Record<string, SentQuestion>>>({});

  apply(event: AiRunEvent): void {
    this.runs = applyRunEvent(this.runs, event);
  }

  remember(runId: string, question: SentQuestion): void {
    this.sent = { ...this.sent, [runId]: question };
  }

  /** Drops a failed Run's banner, e.g. once "Try again" has started another. */
  dismiss(runId: string): void {
    this.runs = Object.fromEntries(Object.entries(this.runs).filter(([id]) => id !== runId));
  }

  clear(): void {
    this.runs = {};
    this.sent = {};
  }
}

export const liveRuns = new LiveRunStore();
