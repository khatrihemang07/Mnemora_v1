import type { ModelUsage, ProviderView } from '../../lib/ipc/bindings';

/** A Provider and the Model asked through it. */
export interface Choice {
  readonly providerId: string;
  readonly model: string;
}

/** Where a default Model can come from, in the order the page prefers them. */
export interface ChoiceSources {
  /** Picked on this page since it opened. */
  picked: Choice | null;
  /** What the open Conversation last used, if one is open. */
  conversation: { providerId: string | null; model: string | null } | null;
  /** What this device has used, most recent first. */
  usage: ModelUsage | null;
  /** The Providers this device has. A choice naming a missing one is not used. */
  providers: readonly ProviderView[];
}

/**
 * The Model a question will be asked with: the one picked here, else the open Conversation's, else
 * the most recent one this device used. `null` means the person must choose one (the pill says so).
 */
export function effectiveChoice(sources: ChoiceSources): Choice | null {
  const live = new Set(sources.providers.map((provider) => provider.id));
  const usable = (choice: Choice | null): Choice | null =>
    choice && live.has(choice.providerId) && choice.model !== '' ? choice : null;
  const conversation = sources.conversation;
  // A Conversation synced from another device may name a Provider this device lacks. Then the
  // person chooses one of this device's own: this device's usual Model is not used in its place.
  const lacksProvider = !!conversation?.providerId && !live.has(conversation.providerId);
  const fromConversation =
    conversation?.providerId && conversation.model && !lacksProvider
      ? { providerId: conversation.providerId, model: conversation.model }
      : null;
  const latest = sources.usage?.recent[0];
  const fromUsage =
    latest && !lacksProvider ? { providerId: latest.provider_id, model: latest.model } : null;
  return usable(sources.picked) ?? usable(fromConversation) ?? usable(fromUsage);
}
