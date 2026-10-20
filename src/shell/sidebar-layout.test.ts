import { describe, expect, it } from 'vitest';
import { draggedWidth } from '../lib/ui/pane-math';
import {
  clampWidth,
  DEFAULT_WIDTH,
  RAIL_WIDTH,
  resetLayout,
  settleDrag,
  toggleRail,
} from './sidebar-layout';

describe('clampWidth', () => {
  it('keeps values inside 200..360', () => {
    expect(clampWidth(150)).toBe(200);
    expect(clampWidth(250)).toBe(250);
    expect(clampWidth(500)).toBe(360);
  });
});

describe('settleDrag', () => {
  it('snaps to the rail below 150px', () => {
    expect(settleDrag(149, { width: 260, rail: false })).toEqual({ width: 260, rail: true });
  });
  it('clamps 150..200 up to 200', () => {
    expect(settleDrag(170, { width: 260, rail: false })).toEqual({ width: 200, rail: false });
  });
  it('expands from the rail only past 150px', () => {
    expect(settleDrag(100, { width: 260, rail: true })).toEqual({ width: 260, rail: true });
    expect(settleDrag(300, { width: 260, rail: true })).toEqual({ width: 300, rail: false });
  });
  it('clamps the top end', () => {
    expect(settleDrag(900, { width: 260, rail: false })).toEqual({ width: 360, rail: false });
  });
});

describe('toggleRail / reset', () => {
  it('toggle keeps the remembered width', () => {
    expect(toggleRail({ width: 300, rail: false })).toEqual({ width: 300, rail: true });
    expect(toggleRail({ width: 300, rail: true })).toEqual({ width: 300, rail: false });
  });
  it('reset goes back to 248 expanded', () => {
    expect(resetLayout()).toEqual({ width: DEFAULT_WIDTH, rail: false });
    expect(DEFAULT_WIDTH).toBe(248);
    expect(RAIL_WIDTH).toBe(60);
  });
});

describe('drag release end to end', () => {
  const cur = { width: 248, rail: false };
  const release = (percent: number) => settleDrag(draggedWidth(percent, 1000, 248) as number, cur);
  it('160px clamps to 200, 100px snaps to the rail, 300px is free', () => {
    expect(release(16)).toEqual({ width: 200, rail: false });
    expect(release(10)).toEqual({ width: 248, rail: true });
    expect(release(30)).toEqual({ width: 300, rail: false });
  });
});
