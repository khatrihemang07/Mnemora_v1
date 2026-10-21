import type { AllView, Task, TodayView, UpcomingView } from '../../lib/ipc/bindings';
import { plainTitle } from './notes';

/** Longest title shown in a toast before it is cut with an ellipsis. */
const TOAST_TITLE_CHARS = 34;

/** A name or title in curly quotes, cut at 34 characters for a toast. */
export function quoted(text: string): string {
  const chars = [...text];
  const shown = chars.slice(0, TOAST_TITLE_CHARS).join('').trimEnd();
  return `“${shown}${chars.length > TOAST_TITLE_CHARS ? '…' : ''}”`;
}

/** A Task's title for a toast: its Markdown marks removed, in curly quotes, cut at 34 characters. */
export const quotedTitle = (title: string): string => quoted(plainTitle(title));

/** The Undo toast after a tick: names the Task, e.g. `Completed “Buy oat milk”`. */
export const completedMessage = (title: string): string => `Completed ${quotedTitle(title)}`;

/** A page of Tasks (the Inbox, a List) without one Task: what it shows the instant it is ticked. */
export function withoutTask<V extends { tasks: Task[] }>(view: V, id: string): V {
  return { ...view, tasks: view.tasks.filter((t) => t.id !== id) };
}

/** The All view without one Task; a group left empty goes with it. */
export function withoutTaskInAll(view: AllView, id: string): AllView {
  return {
    groups: view.groups
      .map((g) => ({ ...g, tasks: g.tasks.filter((t) => t.id !== id) }))
      .filter((g) => g.tasks.length > 0),
  };
}

/** Today without one Task: what it shows the instant that Task is ticked. */
export function withoutTaskInToday(view: TodayView, id: string): TodayView {
  const keep = (t: Task) => t.id !== id;
  return { ...view, overdue: view.overdue.filter(keep), tasks: view.tasks.filter(keep) };
}

/** Upcoming without one Task; a day left empty goes with it. */
export function withoutTaskInUpcoming(view: UpcomingView, id: string): UpcomingView {
  const keep = (t: Task) => t.id !== id;
  return {
    overdue: view.overdue.filter(keep),
    days: view.days
      .map((d) => ({ ...d, tasks: d.tasks.filter(keep) }))
      .filter((d) => d.tasks.length > 0),
  };
}

/** The Undo toast after Won't do: names the Task. */
export const wontMessage = (title: string): string => `Won’t do ${quotedTitle(title)}`;

/** The Undo toast after deleting a Task. */
export const deletedMessage = (title: string): string => `Deleted ${quotedTitle(title)}`;

/** The Undo toast after a Priority change, e.g. `“Pay rent” → P1`. */
export const priorityMessage = (title: string, priority: number): string =>
  `${quotedTitle(title)} → ${priority < 4 ? `P${priority}` : 'No priority'}`;

/** What the checkbox's ring shows for a Priority: 1 red, 2 amber, 3 violet, 4 nothing. */
export const priorityTone = (priority: number): 'p1' | 'p2' | 'p3' | 'plain' =>
  priority === 1 ? 'p1' : priority === 2 ? 'p2' : priority === 3 ? 'p3' : 'plain';

/** The menu and chip name of each Priority. */
export const PRIORITY_NAMES: Record<number, string> = {
  1: 'Priority 1',
  2: 'Priority 2',
  3: 'Priority 3',
  4: 'No priority',
};
