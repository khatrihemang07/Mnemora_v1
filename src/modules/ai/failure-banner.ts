import type { Failure } from '../../lib/ipc/bindings';

/** What a failure banner can offer besides reading the sentence. */
export type BannerAction =
  | 'retry'
  | 'change_model'
  | 'change_provider'
  | 'branch_from_earlier'
  | 'new_conversation';

/** How a failed Run is shown in the answer's place. */
export interface FailureBanner {
  readonly tone: 'danger' | 'warning';
  /** A short heading; the sentence (the approved wording from Rust) is shown below it. */
  readonly title: string;
  /** The actions, first one primary. */
  readonly actions: readonly BannerAction[];
}

/**
 * The banner for each failure kind. The title is a short label; the sentence under it is the one Rust
 * wrote, so the title must not be the start of that sentence (see the failure-banner test).
 * The question and the steps so far stay in the thread.
 */
export function failureBanner(kind: Failure): FailureBanner {
  switch (kind) {
    case 'offline':
      return { tone: 'warning', title: 'No connection', actions: ['retry'] };
    case 'too_long':
      return {
        tone: 'warning',
        title: 'Conversation is full',
        actions: ['branch_from_earlier', 'new_conversation'],
      };
    case 'no_provider':
      return { tone: 'danger', title: 'No Provider yet', actions: ['change_provider'] };
    case 'unreachable':
      return {
        tone: 'danger',
        title: 'Connection failed',
        actions: ['retry', 'change_provider'],
      };
    case 'timeout':
      return {
        tone: 'danger',
        title: 'No answer in time',
        actions: ['retry', 'change_provider'],
      };
    case 'key_rejected':
      return {
        tone: 'danger',
        title: 'Key not accepted',
        actions: ['change_provider', 'retry'],
      };
    case 'rate_limited':
      return { tone: 'danger', title: 'Limit reached', actions: ['retry'] };
    case 'model_unavailable':
      return { tone: 'danger', title: 'Choose another Model', actions: ['change_model'] };
    case 'model_cannot_use_tools':
      return { tone: 'danger', title: 'Choose another Model', actions: ['change_model'] };
    case 'bad_response':
      return { tone: 'danger', title: 'Unreadable reply', actions: ['retry'] };
    case 'interrupted':
      return { tone: 'danger', title: 'Answer stopped', actions: ['retry'] };
    case 'unexpected':
      return { tone: 'danger', title: 'Unexpected error', actions: ['retry'] };
  }
}
