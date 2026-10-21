import type { AllView, Task } from '../../lib/ipc/bindings';

/** Longest title shown in a toast before it is cut with an ellipsis. */
const TOAST_TITLE_CHARS = 34;

/** A name or title in curly quotes, cut at 34 characters for a toast. */
export function quoted(text: string): string {
  const chars = [...text];
  const shown = chars.slice(0, TOAST_TITLE_CHARS).join('').trimEnd();
  return `“${shown}${chars.length > TOAST_TITLE_CHARS ? '…' : ''}”`;
}

/** The Undo toast after a tick: names the Task, e.g. `Completed “Buy oat milk”`. */
export const completedMessage = (title: string): string => `Completed ${quoted(title)}`;

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
