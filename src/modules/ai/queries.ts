import { infiniteQueryOptions, queryOptions } from '@tanstack/svelte-query';
import { commands } from '../../lib/ipc/bindings';
import { unwrap } from '../../lib/ipc/unwrap';

/** Every AI query starts with `ai`, so a change can refresh them all at once. */
/** Conversations per history page. */
export const HISTORY_PAGE = 30;

export const aiKeys = {
  all: ['ai'] as const,
  /** Every history query (any search), refreshed together. */
  history: ['ai', 'conversations'] as const,
  saved: ['ai', 'saved'] as const,
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

/** A Provider's Model ids, and which of them are paid routes (OpenRouter only). */
export interface ProviderModels {
  models: string[];
  paid: string[];
}

/**
 * The full Model list of one Provider. Fetched only when the person browses or searches, then kept
 * for the session: the cache never goes stale and is not dropped while the app runs.
 */
export function modelsOptions(providerId: string) {
  return queryOptions({
    queryKey: aiKeys.models(providerId),
    queryFn: async (): Promise<ProviderModels> => {
      const list = await unwrap(commands.aiModelsSearch(providerId, ''));
      if (list.failure) throw new Error(list.message);
      return { models: list.models, paid: list.paid_models };
    },
    staleTime: Number.POSITIVE_INFINITY,
    gcTime: Number.POSITIVE_INFINITY,
    retry: false,
  });
}

/** The history, a page at a time, newest first, filtered by title. */
export function historyOptions(query: string) {
  return infiniteQueryOptions({
    queryKey: [...aiKeys.history, query] as const,
    queryFn: ({ pageParam }) => unwrap(commands.aiConversationList(query, pageParam, HISTORY_PAGE)),
    initialPageParam: 0,
    getNextPageParam: (last, pages) =>
      last.more ? pages.reduce((total, page) => total + page.items.length, 0) : undefined,
  });
}

/** The Saved questions in their order. */
export const savedOptions = queryOptions({
  queryKey: aiKeys.saved,
  queryFn: () => unwrap(commands.aiSavedList()),
});
