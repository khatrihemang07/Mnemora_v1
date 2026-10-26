/** Which way a Saved question moves in the list. */
export type MoveDirection = 'up' | 'down';

/** The neighbours a Saved question is placed between when it moves one place. */
export interface Neighbours {
  afterId: string | null;
  beforeId: string | null;
}

/**
 * The neighbours to send to `ai_saved_reorder` to move `id` one place: up puts it above the one
 * before it, down below the one after it. Null when it is already at that end.
 */
export function moveNeighbours(
  ids: readonly string[],
  id: string,
  direction: MoveDirection,
): Neighbours | null {
  const i = ids.indexOf(id);
  if (i < 0) return null;
  if (direction === 'up') {
    if (i === 0) return null;
    return { afterId: ids[i - 2] ?? null, beforeId: ids[i - 1] ?? null };
  }
  if (i === ids.length - 1) return null;
  return { afterId: ids[i + 1] ?? null, beforeId: ids[i + 2] ?? null };
}
