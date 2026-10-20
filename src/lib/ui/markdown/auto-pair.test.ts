import { describe, expect, it } from 'vitest';
import { autoPair, pairBackspace } from './auto-pair';
import { applyEdit } from './edit';

/** Types `ch` at `from..to` of `doc`; a null result means "type it normally". */
function type(doc: string, from: number, to: number, ch: string) {
  const edit = autoPair(doc, from, to, ch);
  if (!edit) return null;
  const r = applyEdit(doc, edit);
  return edit.selection
    ? { doc: r.doc, anchor: r.anchor, head: r.head }
    : { doc: r.doc, anchor: from, head: from };
}

describe('autoPair: opening', () => {
  it('closes ( [ and ` with the caret between', () => {
    expect(type('', 0, 0, '(')).toEqual({ doc: '()', anchor: 1, head: 1 });
    expect(type('a ', 2, 2, '[')).toEqual({ doc: 'a []', anchor: 3, head: 3 });
    expect(type('', 0, 0, '`')).toEqual({ doc: '``', anchor: 1, head: 1 });
  });

  it('does not close in front of a word, and not a backtick after a word', () => {
    expect(type('word', 0, 0, '(')).toBeNull();
    expect(type('don', 3, 3, '`')).toBeNull();
  });

  it('closes in front of a space or closing punctuation', () => {
    expect(type('a b', 1, 1, '(')).toEqual({ doc: 'a() b', anchor: 2, head: 2 });
    expect(type('(a)', 2, 2, '[')).toEqual({ doc: '(a[])', anchor: 3, head: 3 });
  });

  it('a second * turns into a bold pair, a single * is plain', () => {
    expect(type('', 0, 0, '*')).toBeNull();
    expect(type('*', 1, 1, '*')).toEqual({ doc: '****', anchor: 2, head: 2 });
    expect(type('**', 2, 2, '*')).toBeNull();
  });

  it('three backticks stay three (a code fence)', () => {
    expect(type('``', 2, 2, '`')).toBeNull();
  });

  it('never pairs quotes or underscores', () => {
    for (const ch of ['"', "'", '_', '{']) expect(type('', 0, 0, ch)).toBeNull();
  });
});

describe('autoPair: wrapping a selection', () => {
  it('wraps and keeps the text selected', () => {
    expect(type('say hi', 4, 6, '(')).toEqual({ doc: 'say (hi)', anchor: 5, head: 7 });
    expect(type('say hi', 4, 6, '`')).toEqual({ doc: 'say `hi`', anchor: 5, head: 7 });
    expect(type('say hi', 4, 6, '[')).toEqual({ doc: 'say [hi]', anchor: 5, head: 7 });
  });

  it('typing * twice around a selection makes it bold', () => {
    const first = type('hi', 0, 2, '*');
    expect(first).toEqual({ doc: '*hi*', anchor: 1, head: 3 });
    expect(type('*hi*', 1, 3, '*')?.doc).toBe('**hi**');
  });

  it('other characters replace the selection as usual', () => {
    expect(type('abc', 0, 3, 'x')).toBeNull();
    expect(type('abc', 0, 3, '"')).toBeNull();
  });
});

describe('autoPair: stepping over a closer', () => {
  it('types over ) and ]', () => {
    expect(type('(a)', 2, 2, ')')).toEqual({ doc: '(a)', anchor: 3, head: 3 });
    expect(type('[a]', 2, 2, ']')).toEqual({ doc: '[a]', anchor: 3, head: 3 });
    expect(type('(a', 2, 2, ')')).toBeNull();
  });

  it('types over a closing backtick and closing **', () => {
    expect(type('`a`', 2, 2, '`')?.anchor).toBe(3);
    expect(type('**a**', 3, 3, '*')?.anchor).toBe(4);
    expect(type('**a**', 4, 4, '*')?.anchor).toBe(5);
  });

  it('does not swallow a * that opens a new span', () => {
    expect(type('a *b*', 2, 2, '*')).toBeNull();
    expect(type('*b*', 0, 0, '*')).toBeNull();
  });
});

describe('pairBackspace', () => {
  const back = (doc: string, pos: number) => {
    const e = pairBackspace(doc, pos);
    return e ? applyEdit(doc, e).doc : null;
  };

  it('deletes an empty pair', () => {
    expect(back('a()b', 2)).toBe('ab');
    expect(back('[]', 1)).toBe('');
    expect(back('``', 1)).toBe('');
    expect(back('x**x', 2)).toBe('xx');
    expect(back('****', 2)).toBe('');
  });

  it('leaves non-empty or unmatched pairs to the normal Backspace', () => {
    expect(back('(a)', 2)).toBeNull();
    expect(back('(]', 1)).toBeNull();
    expect(back('abc', 1)).toBeNull();
    expect(back('***', 1)).toBeNull();
  });
});
