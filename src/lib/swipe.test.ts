import { describe, expect, it } from 'vitest';
import {
  lockAxis,
  ROW_COMMIT_PX,
  rowOffset,
  rowSwipeOutcome,
  sheetShouldClose,
  viewSwipeStep,
} from './swipe';

const both = { right: true, left: true };

describe('lockAxis', () => {
  it('waits inside the dead zone', () => {
    expect(lockAxis(3, 3)).toBeNull();
  });
  it('locks sideways only when clearly sideways', () => {
    expect(lockAxis(20, 4)).toBe('x');
    expect(lockAxis(-20, 10)).toBe('x');
    expect(lockAxis(10, 10)).toBe('y');
    expect(lockAxis(4, 20)).toBe('y');
  });
});

describe('row swipes', () => {
  it('follows the finger up to the commit distance, then resists', () => {
    expect(rowOffset(40, both)).toBe(40);
    expect(rowOffset(ROW_COMMIT_PX, both)).toBe(ROW_COMMIT_PX);
    expect(rowOffset(ROW_COMMIT_PX + 100, both)).toBeLessThan(ROW_COMMIT_PX + 100);
    expect(rowOffset(-(ROW_COMMIT_PX + 100), both)).toBeLessThan(-ROW_COMMIT_PX);
  });
  it('does not move towards a side with no action', () => {
    expect(rowOffset(50, { right: false, left: true })).toBe(0);
    expect(rowOffset(-50, { right: true, left: false })).toBe(0);
  });
  it('commits right and left past the distance only', () => {
    expect(rowSwipeOutcome(ROW_COMMIT_PX, both)).toBe('right');
    expect(rowSwipeOutcome(-ROW_COMMIT_PX, both)).toBe('left');
    expect(rowSwipeOutcome(ROW_COMMIT_PX - 1, both)).toBeNull();
    expect(rowSwipeOutcome(200, { right: false, left: true })).toBeNull();
  });
});

describe('viewSwipeStep', () => {
  it('goes to the next view on a left swipe and the previous on a right one', () => {
    expect(viewSwipeStep(-80, 5)).toBe(1);
    expect(viewSwipeStep(80, -5)).toBe(-1);
  });
  it('ignores short or slanted swipes', () => {
    expect(viewSwipeStep(-50, 0)).toBe(0);
    expect(viewSwipeStep(-90, 60)).toBe(0);
  });
});

describe('sheetShouldClose', () => {
  it('closes on a long pull or a quick flick', () => {
    expect(sheetShouldClose(120, 800)).toBe(true);
    expect(sheetShouldClose(60, 80)).toBe(true);
  });
  it('stays on a short slow pull', () => {
    expect(sheetShouldClose(60, 600)).toBe(false);
    expect(sheetShouldClose(10, 5)).toBe(false);
  });
});
