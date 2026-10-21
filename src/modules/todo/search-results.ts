import type { Span, TaskHit } from '../../lib/ipc/bindings';
import { foldWord } from '../../lib/ui/mark-words';

/** Search runs this long after the last keystroke. */
export const SEARCH_DEBOUNCE_MS = 150;

/** Whether the words hold anything to look for (a letter or a digit). */
export const canSearch = (query: string): boolean => /[\p{L}\p{N}]/u.test(query);

/** Open Tasks first, then Done and Won't do ones, each in the order Rust returned them. */
export function splitHits(hits: TaskHit[]): { open: TaskHit[]; done: TaskHit[] } {
  return {
    open: hits.filter((h) => h.task.state === 'open'),
    done: hits.filter((h) => h.task.state !== 'open'),
  };
}

/** The matched words of a title, folded, for tinting them wherever the title is drawn. */
export const litWords = (spans: Span[]): Set<string> =>
  new Set(spans.filter((s) => s.matched).map((s) => foldWord(s.text)));

/** The snippet as plain text, for a row's accessible name. */
export const spansText = (spans: Span[]): string => spans.map((s) => s.text).join('');

/** "1 result", "12 results". */
export const resultsLabel = (n: number): string => `${n} result${n === 1 ? '' : 's'}`;
