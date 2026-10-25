import type { Failure } from '../../lib/ipc/bindings';

/** What a failure banner can offer besides reading the sentence. */
export type BannerAction = 'retry' | 'change_model' | 'change_provider' | 'new_conversation';

/** How a failed Run is shown in the answer's place. */
export interface FailureBanner {
  readonly tone: 'danger' | 'warning';
  /** A short heading; the sentence (the approved wording from Rust) is shown below it. */
  readonly title: string;
  /** The actions, first one primary. */
  readonly actions: readonly BannerAction[];
}

/** The banner for each failure kind. The question and the steps so far stay in the thread. */
export function failureBanner(kind: Failure): FailureBanner {
  switch (kind) {
    case 'offline':
      return { tone: 'warning', title: 'No connection', actions: ['retry'] };
    case 'too_long':
      return { tone: 'warning', title: 'Too long to continue here', actions: ['new_conversation'] };
    case 'no_provider':
      return { tone: 'danger', title: 'No Provider yet', actions: ['change_provider'] };
    case 'unreachable':
      return {
        tone: 'danger',
        title: 'Could not reach the AI service',
        actions: ['retry', 'change_provider'],
      };
    case 'timeout':
      return {
        tone: 'danger',
        title: 'The AI service did not answer',
        actions: ['retry', 'change_provider'],
      };
    case 'key_rejected':
      return {
        tone: 'danger',
        title: 'The key was rejected',
        actions: ['change_provider', 'retry'],
      };
    case 'rate_limited':
      return { tone: 'danger', title: 'The AI service is busy', actions: ['retry'] };
    case 'model_unavailable':
      return { tone: 'danger', title: 'That Model is not available', actions: ['change_model'] };
    case 'model_cannot_use_tools':
      return { tone: 'danger', title: 'That Model cannot use Tools', actions: ['change_model'] };
    case 'bad_response':
      return { tone: 'danger', title: 'The reply could not be read', actions: ['retry'] };
    case 'interrupted':
      return { tone: 'danger', title: 'The answer was interrupted', actions: ['retry'] };
    case 'unexpected':
      return { tone: 'danger', title: 'Something went wrong', actions: ['retry'] };
  }
}
