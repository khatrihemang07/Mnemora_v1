import type { ConversationSummary } from '../../lib/ipc/bindings';

/** The groups of the history, in the order they show. */
export type HistoryGroup = 'today' | 'yesterday' | 'previous7' | 'earlier';

export const GROUP_LABELS: Record<HistoryGroup, string> = {
  today: 'Today',
  yesterday: 'Yesterday',
  previous7: 'Previous 7 days',
  earlier: 'Earlier',
};

const DAY_MS = 86_400_000;

/** The local calendar day of an instant, as midnight in milliseconds. */
function startOfDay(instant: number | Date): number {
  const d = instant instanceof Date ? instant : new Date(instant);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** When a Conversation last changed, in milliseconds (the binding types it as nullable). */
export const updatedAt = (c: ConversationSummary): number => c.updated_at ?? 0;

/**
 * The group a Conversation changed in falls into, by whole local days from `now`. Rounding absorbs
 * the 23 and 25 hour days of a clock change; a time ahead of `now` counts as today.
 */
export function groupOf(updatedAt: number, now: Date): HistoryGroup {
  const days = Math.round((startOfDay(now) - startOfDay(updatedAt)) / DAY_MS);
  if (days <= 0) return 'today';
  if (days === 1) return 'yesterday';
  if (days <= 7) return 'previous7';
  return 'earlier';
}

export interface HistorySection {
  group: HistoryGroup;
  label: string;
  items: ConversationSummary[];
}

/** The non-empty groups of `items` (kept in their order), each with its label. */
export function groupConversations(
  items: readonly ConversationSummary[],
  now: Date,
): HistorySection[] {
  const order: HistoryGroup[] = ['today', 'yesterday', 'previous7', 'earlier'];
  return order
    .map((group) => ({
      group,
      label: GROUP_LABELS[group],
      items: items.filter((c) => groupOf(updatedAt(c), now) === group),
    }))
    .filter((section) => section.items.length > 0);
}

/** A row's date: "Oct 18", the day it last changed. */
export function shortDate(updatedAt: number): string {
  return new Date(updatedAt).toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
}
