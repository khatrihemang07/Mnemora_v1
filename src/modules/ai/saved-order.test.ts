import { describe, expect, it } from 'vitest';
import { moveNeighbours } from './saved-order';

describe('moveNeighbours', () => {
  const ids = ['A', 'B', 'C'];

  it('places a question above the one before its neighbour when it moves up', () => {
    expect(moveNeighbours(ids, 'C', 'up')).toEqual({ afterId: 'A', beforeId: 'B' });
    expect(moveNeighbours(ids, 'B', 'up')).toEqual({ afterId: null, beforeId: 'A' });
  });

  it('places a question below the one after its neighbour when it moves down', () => {
    expect(moveNeighbours(ids, 'A', 'down')).toEqual({ afterId: 'B', beforeId: 'C' });
    expect(moveNeighbours(ids, 'B', 'down')).toEqual({ afterId: 'C', beforeId: null });
  });

  it('refuses to move past either end, or an unknown question', () => {
    expect(moveNeighbours(ids, 'A', 'up')).toBeNull();
    expect(moveNeighbours(ids, 'C', 'down')).toBeNull();
    expect(moveNeighbours(ids, 'Z', 'up')).toBeNull();
  });
});
