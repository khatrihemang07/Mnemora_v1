import { describe, expect, it } from 'vitest';
import {
  activeModuleId,
  isMoreActive,
  MAX_TABS,
  moveTab,
  toggleTab,
  visibleTabs,
} from './phone-tabs';

describe('toggleTab', () => {
  it('adds and removes', () => {
    expect(toggleTab(['home'], 'settings')).toEqual(['home', 'settings']);
    expect(toggleTab(['home', 'settings'], 'home')).toEqual(['settings']);
  });
  it('refuses a fifth tab', () => {
    const four = ['a', 'b', 'c', 'd'];
    expect(MAX_TABS).toBe(4);
    expect(toggleTab(four, 'e')).toBe(four);
  });
});

describe('moveTab', () => {
  it('moves up and down', () => {
    expect(moveTab(['a', 'b', 'c'], 'b', -1)).toEqual(['b', 'a', 'c']);
    expect(moveTab(['a', 'b', 'c'], 'b', 1)).toEqual(['a', 'c', 'b']);
  });
  it('is a no-op at the edges', () => {
    expect(moveTab(['a', 'b'], 'a', -1)).toEqual(['a', 'b']);
    expect(moveTab(['a', 'b'], 'b', 1)).toEqual(['a', 'b']);
  });
});

describe('visibleTabs', () => {
  it('keeps order, drops unknown ids, caps at 4', () => {
    const known = ['home', 'settings', 'todo'];
    expect(visibleTabs(['settings', 'ghost', 'home'], known)).toEqual(['settings', 'home']);
    expect(visibleTabs(['a', 'b', 'c', 'd', 'e'], ['a', 'b', 'c', 'd', 'e'])).toHaveLength(4);
  });
});

const entries = [
  { id: 'home', path: '/' },
  { id: 'settings', path: '/settings' },
  { id: 'todo', path: '/todo' },
];

describe('activeModuleId', () => {
  it('matches exact and nested paths, and "/" only exactly', () => {
    expect(activeModuleId('/', entries)).toBe('home');
    expect(activeModuleId('/settings', entries)).toBe('settings');
    expect(activeModuleId('/todo/42', entries)).toBe('todo');
    expect(activeModuleId('/nowhere', entries)).toBeUndefined();
  });
});

describe('isMoreActive', () => {
  it('is on for Settings or any Module not pinned to a tab', () => {
    expect(isMoreActive('settings', ['home', 'todo'])).toBe(true);
    expect(isMoreActive('todo', ['home'])).toBe(true);
  });
  it('is off for pinned Modules and unknown routes', () => {
    expect(isMoreActive('home', ['home'])).toBe(false);
    expect(isMoreActive(undefined, ['home'])).toBe(false);
  });
});
