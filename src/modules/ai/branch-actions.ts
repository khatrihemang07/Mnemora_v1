import { commands } from '../../lib/ipc/bindings';
import { queryClient } from '../../lib/ipc/query-client';
import { unwrap } from '../../lib/ipc/unwrap';
import { aiKeys } from './queries';

/**
 * Branch from earlier: the Branch that ends at `answerId` is shown, so the next question continues
 * from it. The Branch after it stays in the Conversation.
 */
export async function branchFromEarlier(conversationId: string, answerId: string): Promise<void> {
  await unwrap(commands.aiConversationSetLeaf(conversationId, answerId));
  void queryClient.invalidateQueries({ queryKey: aiKeys.conversation(conversationId) });
  void queryClient.invalidateQueries({ queryKey: aiKeys.history });
}

/** The pager: shows the Branch of the question at `position` (from 1) among those beside `questionId`. */
export async function showBranch(
  conversationId: string,
  questionId: string,
  position: number,
): Promise<void> {
  await unwrap(commands.aiConversationShowBranch(conversationId, questionId, position));
  void queryClient.invalidateQueries({ queryKey: aiKeys.conversation(conversationId) });
}
