import type { Hit, Span } from '../../lib/ipc/bindings';
import { addDays, shiftMonthKeepingDay } from '../../lib/ui/calendar';
import { dayLabel } from './dates';

/** Search runs this long after the last keystroke. */
export const SEARCH_DEBOUNCE_MS = 150;

export type PresetId = 'week' | 'month' | 'year';

export const PRESETS: { id: PresetId; label: string }[] = [
  { id: 'week', label: 'Past week' },
  { id: 'month', label: 'Past month' },
  { id: 'year', label: 'Past year' },
];

/** A Page-date range, ends included; `preset` names it when it came from the menu. */
export interface SearchRange {
  from: string;
  to: string;
  preset: PresetId | null;
}

/** The same calendar day a week, month or year before `today`, up to `today`. */
export function presetRange(id: PresetId, today: string): SearchRange {
  const from =
    id === 'week' ? addDays(today, -7) : shiftMonthKeepingDay(today, id === 'month' ? -1 : -12);
  return { from, to: today, preset: id };
}

/** A short day for the chip: "15 Oct", with the year when it is not `today`'s year. */
function chipDay(date: string, today: string): string {
  const [y = 1970, m = 1, d = 1] = date.split('-').map(Number);
  const text = new Intl.DateTimeFormat('en-GB', {
    day: 'numeric',
    month: 'short',
    timeZone: 'UTC',
  }).format(new Date(Date.UTC(y, m - 1, d)));
  return date.slice(0, 4) === today.slice(0, 4) ? text : `${text} ${y}`;
}

/** What the date chip says: "Any date", the preset's name, or the custom dates. */
export function rangeLabel(range: SearchRange | null, today: string): string {
  if (!range) return 'Any date';
  const preset = PRESETS.find((p) => p.id === range.preset);
  if (preset) return preset.label;
  if (range.from === range.to) return chipDay(range.from, today);
  return `${chipDay(range.from, today)} – ${chipDay(range.to, today)}`;
}

/** What the user is searching for. */
export interface SearchFilter {
  query: string;
  range: SearchRange | null;
}

/** Whether there is anything to ask the Journal: words, or a range to browse. */
export const canSearch = (f: SearchFilter) => f.query.trim() !== '' || f.range !== null;

/**
 * The search view's lifecycle. `parked` holds the filter of a search that was left by opening a
 * result: Back or ⌘F resumes it; any other way out clears it.
 */
export interface SearchSession {
  active: boolean;
  filter: SearchFilter;
  parked: SearchFilter | null;
}

const FRESH: SearchFilter = { query: '', range: null };

export const IDLE: SearchSession = { active: false, filter: FRESH, parked: null };

/** Opens search: back to the parked results when there are some, else a blank search. */
export function openSearch(s: SearchSession): SearchSession {
  if (s.active) return s;
  return { active: true, filter: s.parked ?? FRESH, parked: null };
}

/** Opens a blank search holding `query` (⌘K's "Search Journal for…"). */
export function openSearchFor(query: string): SearchSession {
  return { active: true, filter: { query, range: null }, parked: null };
}

/** Leaves search for good: the query and the date filter are cleared. */
export const closeSearch = (): SearchSession => IDLE;

/** Opening a result closes search but remembers it, so Back can return. */
export function openResult(s: SearchSession): SearchSession {
  return s.active ? { active: false, filter: FRESH, parked: s.filter } : s;
}

/** Back (or ⌘F) returns to parked results; otherwise it leaves search. */
export function goBack(s: SearchSession): SearchSession {
  return s.parked ? openSearch(s) : closeSearch();
}

/** Whether Back means something to search right now. */
export const wantsBack = (s: SearchSession) => s.active || s.parked !== null;

/** Esc: clears the words first, then closes. */
export function escapeSearch(s: SearchSession): SearchSession {
  return s.filter.query === '' ? closeSearch() : { ...s, filter: { ...s.filter, query: '' } };
}

export const withQuery = (s: SearchSession, query: string): SearchSession => ({
  ...s,
  filter: { ...s.filter, query },
});

export const withRange = (s: SearchSession, range: SearchRange | null): SearchSession => ({
  ...s,
  filter: { ...s.filter, range },
});

/** Hits that share a Page, in the order given (the Rust side sorts them). */
export interface HitGroup {
  date: string;
  /** "Thu 15 Oct", with the year when it is not this year. */
  label: string;
  /** Position of its first hit in the whole list: `first` plus the hits before it. */
  start: number;
  hits: Hit[];
}

/** Groups `hits` by Page; `first` is the list position of `hits[0]` (the keyboard highlight index). */
export function groupHits(hits: Hit[], today: string, first = 0): HitGroup[] {
  const groups: HitGroup[] = [];
  for (const [i, hit] of hits.entries()) {
    const last = groups.at(-1);
    if (last?.date === hit.page_date) last.hits.push(hit);
    else {
      const label = dayLabel(hit.page_date, today);
      groups.push({ date: hit.page_date, label, start: first + i, hits: [hit] });
    }
  }
  return groups;
}

/** DOM id of a result row, for `aria-activedescendant`. */
export const hitDomId = (listId: string, hit: Hit) => `${listId}-${hit.id}`;

/** "3 entries", "1 entry". */
export const entriesLabel = (n: number) => `${n} ${n === 1 ? 'entry' : 'entries'}`;

/** Snippet text for a screen reader: the spans joined, with no marks. */
export const snippetText = (spans: Span[]) => spans.map((s) => s.text).join('');

/** Where an arrow key moves the highlighted result (-1 is none, the field). */
export function moveActive(index: number, key: 'ArrowDown' | 'ArrowUp', count: number): number {
  if (count === 0) return -1;
  return key === 'ArrowDown' ? Math.min(index + 1, count - 1) : Math.max(index - 1, -1);
}
