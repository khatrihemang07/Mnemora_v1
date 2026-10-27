import type { QueuedNote } from '../../lib/ipc/bindings';

/** The notes waiting for each Conversation's Run (or left over when it ended), by Conversation id. */
export type QueuedNotes = Readonly<Record<string, readonly QueuedNote[]>>;

/** Replaces one Conversation's notes with the list the Run announced. */
export function setQueued(
  queued: QueuedNotes,
  conversationId: string,
  notes: readonly QueuedNote[],
): QueuedNotes {
  return { ...queued, [conversationId]: notes };
}

/** Removes one note from a Conversation's list, once it is sent or taken back. */
export function dropQueued(
  queued: QueuedNotes,
  conversationId: string,
  noteId: string,
): QueuedNotes {
  const kept = (queued[conversationId] ?? []).filter((note) => note.id !== noteId);
  return { ...queued, [conversationId]: kept };
}
