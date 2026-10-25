import type { AiRunEvent } from '../../lib/ipc/bindings';
import { applyRunEvent, type LiveRuns } from './run-state';

/**
 * The Runs the open AI page has seen. In memory only: a reload forgets them, and the saved
 * Conversation is the record.
 */
class LiveRunStore {
  runs = $state<LiveRuns>({});

  apply(event: AiRunEvent): void {
    this.runs = applyRunEvent(this.runs, event);
  }

  clear(): void {
    this.runs = {};
  }
}

export const liveRuns = new LiveRunStore();
