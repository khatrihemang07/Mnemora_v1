import { describe, expect, it } from 'vitest';
import { splitPath } from './path';

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
