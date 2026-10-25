import { queryOptions } from '@tanstack/svelte-query';
import { commands } from '../../lib/ipc/bindings';
import { unwrap } from '../../lib/ipc/unwrap';

/** Every AI query starts with `ai`, so a change can refresh them all at once. */
export const aiKeys = {
  all: ['ai'] as const,
  conversation: (id: string) => ['ai', 'conversation', id] as const,
};

/** A Conversation with its entries. Refreshed when a Run saves an entry or ends. */
export function conversationOptions(id: string) {
  return queryOptions({
    queryKey: aiKeys.conversation(id),
    queryFn: () => unwrap(commands.aiConversationGet(id)),
  });
}
