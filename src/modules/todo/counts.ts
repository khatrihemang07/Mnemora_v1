import type { Task, TodoCounts } from '../../lib/ipc/bindings';
import type { TimeFormat } from '../../lib/time/format';
import type { CaptureFields } from './capture';
import { rowDateLabel, whenChipLabel } from './dates';
import { plainTitle } from './notes';

/** Todo's badge in the sidebar and the phone tab bar. */
export interface Badge {
  count: number;
  /** Something is Overdue: the badge is red. */
  alert: boolean;
}

/** Due today plus Overdue, red while anything is Overdue; none when there is nothing to show. */
export function badgeOf(
  counts: Pick<TodoCounts, 'due_today' | 'overdue'> | undefined,
): Badge | null {
  if (!counts) return null;
  const count = counts.due_today + counts.overdue;
  return count > 0 ? { count, alert: counts.overdue > 0 } : null;
}

/** What Todo's card on Home shows. */
export interface HomeCardModel {
  /** Up to five open Tasks due today. */
  tasks: Task[];
  /** Open Tasks due before today. */
  overdue: number;
  /** Tasks due today beyond the ones shown. */
  more: number;
  /** The line shown instead of Tasks when none is due today, or none. */
  nothing: string | null;
}

/** "Nothing due today · next: Water the plants, Fri", with the hint left off when nothing is coming. */
export function nothingDueText(next: Task | null, today: string): string {
  if (!next?.due_date) return 'Nothing due today';
  return `Nothing due today · next: ${plainTitle(next.title)}, ${rowDateLabel(next.due_date, today)}`;
}

export function homeCardModel(counts: TodoCounts, today: string): HomeCardModel {
  const tasks = counts.card;
  return {
    tasks,
    overdue: counts.overdue,
    more: Math.max(0, counts.due_today - tasks.length),
    nothing: counts.due_today === 0 ? nothingDueText(counts.next, today) : null,
  };
}

/** The "2 overdue · +3 more" footer, as parts so the overdue one can be red; none when empty. */
export function footerParts(model: Pick<HomeCardModel, 'overdue' | 'more'>): {
  overdue: string | null;
  more: string | null;
} {
  return {
    overdue: model.overdue > 0 ? `${model.overdue} overdue` : null,
    more: model.more > 0 ? `+${model.more} more` : null,
  };
}

/**
 * How ⌘K's "Add task" says it understood the text, e.g. `Tomorrow · 18:00 · P2 · Home`; a Task
 * with nothing read out of it goes to the Inbox.
 */
export function captureSummary(
  fields: CaptureFields,
  listName: string | null,
  today: string,
  format: TimeFormat,
): string {
  const when = whenChipLabel(
    {
      due_date: fields.date,
      due_time: fields.time,
      repeat_freq: fields.repeat?.freq ?? null,
      repeat_every: fields.repeat?.every ?? null,
      repeat_days: fields.repeat?.days ?? null,
      repeat_mode: fields.repeat?.mode ?? null,
    },
    today,
    format,
  );
  const parts = [
    when === 'When' ? null : when,
    fields.priority < 4 ? `P${fields.priority}` : null,
    fields.newList !== null ? `New list ${fields.newList}` : listName,
  ];
  return parts.filter((p) => p !== null).join(' · ') || 'Inbox';
}
