import { commands } from '../../lib/ipc/bindings';
import { unwrap } from '../../lib/ipc/unwrap';
import { notify, notifyError } from '../../lib/ui/toast';

/**
 * Copies a Conversation's visible branch, or one answer when `entryId` is given, as Markdown. The
 * Rust side makes the text, so the copy is the same however it is asked for.
 */
export async function copyAsMarkdown(
  conversationId: string,
  entryId: string | null,
): Promise<void> {
  try {
    const markdown = await unwrap(commands.aiConversationMarkdown(conversationId, entryId));
    await navigator.clipboard.writeText(markdown);
    notify(entryId ? 'Answer copied as Markdown' : 'Conversation copied as Markdown');
  } catch (failure) {
    notifyError(failure instanceof Error ? failure.message : 'Could not copy as Markdown.');
  }
}
