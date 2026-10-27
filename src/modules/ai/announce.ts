import type { RunOutcome } from '../../lib/ipc/bindings';

/**
 * What the polite live region says once when a Run ends. Streamed text is never announced token by
 * token: this one sentence is the whole announcement, for every way a Run can end.
 */
export function announcementFor(outcome: RunOutcome): string {
  switch (outcome.type) {
    case 'answered':
      return 'Response complete';
    case 'stopped':
      return 'Response stopped';
    case 'failed':
      return `Response failed: ${outcome.message}`;
  }
}
