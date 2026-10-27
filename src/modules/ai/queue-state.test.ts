import { describe, expect, it } from 'vitest';
import type { QueuedNote } from '../../lib/ipc/bindings';
import { dropQueued, type QueuedNotes, setQueued } from './queue-state';

const note = (id: string, text: string): QueuedNote => ({ id, text });

describe('queued notes', () => {
  it('replaces a Conversation list with the one the Run announced and leaves the others alone', () => {
    const before: QueuedNotes = { c2: [note('n9', 'elsewhere')] };
    const after = setQueued(before, 'c1', [note('n1', 'first'), note('n2', 'second')]);
    expect(after.c1).toEqual([note('n1', 'first'), note('n2', 'second')]);
    expect(after.c2).toEqual([note('n9', 'elsewhere')]);
    expect(before.c1).toBeUndefined();
  });

  it('drops one note when it is sent or taken back, and keeps the rest in order', () => {
    const queued = setQueued({}, 'c1', [note('n1', 'a'), note('n2', 'b'), note('n3', 'c')]);
    const after = dropQueued(queued, 'c1', 'n2');
    expect(after.c1?.map((n) => n.id)).toEqual(['n1', 'n3']);
    expect(dropQueued(after, 'c9', 'n1').c9).toEqual([]);
  });
});
