import { describe, expect, it } from 'vitest';
import { isWithin, splitPath } from './path';

describe('splitPath', () => {
  it('leaves a plain path alone', () => {
    expect(splitPath('/todo')).toEqual({ pathname: '/todo', search: '' });
  });

  it('separates the query', () => {
    expect(splitPath('/todo?task=T1')).toEqual({ pathname: '/todo', search: 'task=T1' });
  });

  it('keeps everything after the first question mark in the query', () => {
    expect(splitPath('/todo?a=1?b=2')).toEqual({ pathname: '/todo', search: 'a=1?b=2' });
  });
});

describe('isWithin', () => {
  it('matches the path itself and the pages below it', () => {
    expect(isWithin('/sync', '/sync')).toBe(true);
    expect(isWithin('/sync/activity', '/sync')).toBe(true);
  });

  it('does not match a sibling that merely starts with the same letters', () => {
    expect(isWithin('/syncing', '/sync')).toBe(false);
    expect(isWithin('/settings', '/sync')).toBe(false);
  });

  it('keeps Home to itself', () => {
    expect(isWithin('/', '/')).toBe(true);
    expect(isWithin('/sync', '/')).toBe(false);
  });
});
