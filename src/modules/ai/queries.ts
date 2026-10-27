import { queryOptions } from '@tanstack/svelte-query';
import { commands } from '../../lib/ipc/bindings';
import { unwrap } from '../../lib/ipc/unwrap';

/** Every AI query starts with `ai`, so a change can refresh them all at once. */
export const aiKeys = {
  all: ['ai'] as const,
  conversation: (id: string) => ['ai', 'conversation', id] as const,
  providers: ['ai', 'providers'] as const,
  usage: ['ai', 'usage'] as const,
  models: (providerId: string) => ['ai', 'models', providerId] as const,
};

/** A Conversation with its entries. Refreshed when a Run saves an entry or ends. */
export function conversationOptions(id: string) {
  return queryOptions({
    queryKey: aiKeys.conversation(id),
    queryFn: () => unwrap(commands.aiConversationGet(id)),
  });
}

/** This device's Providers. Never carries a key. */
export const providersOptions = queryOptions({
  queryKey: aiKeys.providers,
  queryFn: () => unwrap(commands.aiProviderList()),
});

/** The Models this device has used, most recent and most used. */
export const usageOptions = queryOptions({
  queryKey: aiKeys.usage,
  queryFn: () => unwrap(commands.aiModelsUsage()),
});

/**
 * The full Model list of one Provider. Fetched only when the person browses or searches, then kept
 * for the session: the cache never goes stale and is not dropped while the app runs.
 */
export function modelsOptions(providerId: string) {
  return queryOptions({
    queryKey: aiKeys.models(providerId),
    queryFn: async () => {
      const list = await unwrap(commands.aiModelsSearch(providerId, ''));
      if (list.failure) throw new Error(list.message);
      return list.models;
    },
    staleTime: Number.POSITIVE_INFINITY,
    gcTime: Number.POSITIVE_INFINITY,
    retry: false,
  });
}
