import type { InboxView } from '../../lib/ipc/bindings';

/** Longest title shown in a toast before it is cut with an ellipsis. */
const TOAST_TITLE_CHARS = 34;

/** The Undo toast after a tick: names the Task, e.g. `Completed “Buy oat milk”`. */
export function completedMessage(title: string): string {
  const chars = [...title];
  const shown = chars.slice(0, TOAST_TITLE_CHARS).join('').trimEnd();
  return `Completed “${shown}${chars.length > TOAST_TITLE_CHARS ? '…' : ''}”`;
}

/** The Inbox without one Task: what the screen shows the instant it is ticked. */
export function withoutTask(view: InboxView, id: string): InboxView {
  return { ...view, tasks: view.tasks.filter((t) => t.id !== id) };
}
