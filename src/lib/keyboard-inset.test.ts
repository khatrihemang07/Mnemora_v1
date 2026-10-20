import { describe, expect, it } from 'vitest';
import { isKeyboardOpen, keyboardInset } from './keyboard-inset';

describe('keyboardInset', () => {
  it('is the covered height below the visual viewport', () => {
    expect(keyboardInset(800, 500, 0)).toBe(300);
  });
  it('subtracts a panned viewport so the bottom edge stays right', () => {
    expect(keyboardInset(800, 500, 40)).toBe(260);
  });
  it('is zero when nothing covers the page, never negative', () => {
    expect(keyboardInset(800, 800, 0)).toBe(0);
    expect(keyboardInset(800, 810, 0)).toBe(0);
  });
  it('rounds fractional viewports', () => {
    expect(keyboardInset(800, 499.6, 0)).toBe(300);
  });
});

describe('isKeyboardOpen', () => {
  it('ignores small changes such as browser bars', () => {
    expect(isKeyboardOpen(0)).toBe(false);
    expect(isKeyboardOpen(60)).toBe(false);
    expect(isKeyboardOpen(300)).toBe(true);
  });
});
