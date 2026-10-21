import { describe, expect, it } from 'vitest';
import {
  autoScrollSpeed,
  insertIndex,
  isSamePlace,
  neighboursAt,
  type RowBox,
  stepNeighbours,
} from './drag-math';

const rows: RowBox[] = [
  { id: 'a', top: 0, bottom: 40 },
  { id: 'b', top: 40, bottom: 80 },
  { id: 'c', top: 80, bottom: 120 },
];

describe('drop places', () => {
  it('put the pointer above a row in its upper half and below it in its lower half', () => {
    expect(insertIndex(rows, -10)).toBe(0);
    expect(insertIndex(rows, 10)).toBe(0);
    expect(insertIndex(rows, 25)).toBe(1);
    expect(insertIndex(rows, 55)).toBe(1);
    expect(insertIndex(rows, 65)).toBe(2);
    expect(insertIndex(rows, 115)).toBe(3);
    expect(insertIndex(rows, 500)).toBe(3);
    expect(insertIndex([], 20)).toBe(0);
  });

  it('name the Tasks on either side of an index', () => {
    const ids = ['a', 'b', 'c'];
    expect(neighboursAt(ids, 0)).toEqual({ before: null, after: 'a' });
    expect(neighboursAt(ids, 2)).toEqual({ before: 'b', after: 'c' });
    expect(neighboursAt(ids, 3)).toEqual({ before: 'c', after: null });
    expect(neighboursAt([], 0)).toEqual({ before: null, after: null });
  });
});

describe('moving by one place', () => {
  const ids = ['a', 'b', 'c', 'd'];

  it('steps a Task up or down among the others', () => {
    expect(stepNeighbours(ids, 'c', -1)).toEqual({ before: 'a', after: 'b' });
    expect(stepNeighbours(ids, 'b', 1)).toEqual({ before: 'c', after: 'd' });
    expect(stepNeighbours(ids, 'b', -1)).toEqual({ before: null, after: 'a' });
    expect(stepNeighbours(ids, 'c', 1)).toEqual({ before: 'd', after: null });
  });

  it('does not step past either end or for a Task that is not there', () => {
    expect(stepNeighbours(ids, 'a', -1)).toBeNull();
    expect(stepNeighbours(ids, 'd', 1)).toBeNull();
    expect(stepNeighbours(ids, 'x', 1)).toBeNull();
    expect(stepNeighbours(['a'], 'a', 1)).toBeNull();
  });

  it('knows a drop that leaves a Task where it is', () => {
    const ids3 = ['a', 'b', 'c'];
    expect(isSamePlace(ids3, 'b', { before: 'a', after: 'c' })).toBe(true);
    expect(isSamePlace(ids3, 'a', { before: null, after: 'b' })).toBe(true);
    expect(isSamePlace(ids3, 'c', { before: 'b', after: null })).toBe(true);
    expect(isSamePlace(ids3, 'b', { before: 'c', after: null })).toBe(false);
    expect(isSamePlace(ids3, 'x', { before: null, after: null })).toBe(false);
  });
});

describe('scrolling while dragging', () => {
  it('scrolls toward an edge faster the closer the pointer is, and not in the middle', () => {
    expect(autoScrollSpeed(300, 0, 600)).toBe(0);
    expect(autoScrollSpeed(10, 0, 600)).toBeLessThan(0);
    expect(autoScrollSpeed(-20, 0, 600)).toBe(-16);
    expect(autoScrollSpeed(590, 0, 600)).toBeGreaterThan(0);
    expect(autoScrollSpeed(700, 0, 600)).toBe(16);
    expect(Math.abs(autoScrollSpeed(30, 0, 600))).toBeLessThan(
      Math.abs(autoScrollSpeed(5, 0, 600)),
    );
  });
});
