import { describe, expect, it } from 'vitest';
import { paragraphs } from './paragraphs';

describe('paragraphs', () => {
  it('splits on blank lines and keeps single newlines inside a paragraph', () => {
    expect(paragraphs('one\ntwo\n\nthree')).toEqual(['one\ntwo', 'three']);
  });
  it('treats whitespace-only lines as blank and collapses runs of them', () => {
    expect(paragraphs('a\n  \t\n\n\nb')).toEqual(['a', 'b']);
  });
  it('drops leading and trailing blank lines', () => {
    expect(paragraphs('\n\nhello\n\n')).toEqual(['hello']);
  });
  it('is one paragraph for plain text', () => {
    expect(paragraphs('just this')).toEqual(['just this']);
  });
});
