import type { TodaySummary } from '../../lib/ipc/bindings';

/** What the Home card shows for a day's summary. */
export interface TodayCardText {
  /** The latest snippet in curly quotes, or null on an empty day. */
  quote: string | null;
  /** "Today · 3 entries", or "Nothing written today". */
  meta: string;
}

/** The card's words. Rust already made the snippet plain text, so it is only quoted here. */
export function todayCardText(summary: TodaySummary): TodayCardText {
  const snippet = summary.latest?.snippet.trim() ?? '';
  if (summary.count === 0 || snippet === '') {
    return {
      quote: null,
      meta: summary.count === 0 ? 'Nothing written today' : countLabel(summary.count),
    };
  }
  return { quote: `“${snippet}”`, meta: countLabel(summary.count) };
}

const countLabel = (n: number) => `Today · ${n} ${n === 1 ? 'entry' : 'entries'}`;
