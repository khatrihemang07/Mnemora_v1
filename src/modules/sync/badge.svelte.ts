import { createQuery } from '@tanstack/svelte-query';
import type { ModuleBadge } from '../../lib/router';
import { badgeOf } from './attention';
import { attentionQuery } from './queries';

/**
 * Sync's badge for the desktop sidebar: the conflicts that need you, an amber "!" while sync is
 * paused. Call during component setup. SyncOverlay keeps the query fresh while Sync is on.
 */
export function useSyncBadge(): { readonly badge: ModuleBadge | null } {
  const attention = createQuery(() => attentionQuery);
  const badge = $derived(badgeOf(attention.data));
  return {
    get badge() {
      return badge;
    },
  };
}
