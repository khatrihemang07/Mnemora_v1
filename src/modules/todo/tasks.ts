import type { TaskGroup } from '../../lib/ipc/bindings';
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

/**
 * A grouped page (the Inbox, a List, All, Today, Upcoming) without one Task: what it shows the
 * instant that Task is ticked, moved or deleted. A group left empty goes with it, except the plain
 * group, which is where "+ New task" lives.
 */
export function withoutTaskInGroups<V extends { groups: TaskGroup[] }>(view: V, id: string): V {
  const groups = view.groups
    .map((g) => ({ ...g, tasks: g.tasks.filter((t) => t.id !== id) }))
    .filter((g) => g.tasks.length > 0 || g.key.kind === 'plain');
  return { ...view, groups };
}

/** The Undo toast after a Quick edit: names the Task. */
export const editedMessage = (title: string): string => `Edited ${quotedTitle(title)}`;

/** The toast after a Quick add from outside Todo: names the Task and where it went. */
export const quickAddedMessage = (title: string, where: string): string =>
  `Added ${quotedTitle(title)} to ${where}`;

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
