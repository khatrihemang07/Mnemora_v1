import { describe, expect, it } from 'vitest';
import { applyEdit } from './edit';
import { listEnter, listIndent, markerBackspace } from './list-keys';

const enter = (doc: string, pos: number) => {
  const e = listEnter(doc, pos);
  return e ? applyEdit(doc, e) : null;
};

describe('listEnter', () => {
  it('continues a bullet, keeping its character and indent', () => {
    expect(enter('- a', 3)?.doc).toBe('- a\n- ');
    expect(enter('* a', 3)?.doc).toBe('* a\n* ');
    expect(enter('  - a', 5)?.doc).toBe('  - a\n  - ');
  });

  it('counts up numbered items', () => {
    expect(enter('1. a', 4)?.doc).toBe('1. a\n2. ');
    expect(enter('9) a', 4)?.doc).toBe('9) a\n10) ');
  });

  it('continues a checklist with a fresh unticked box, even after a ticked item', () => {
    expect(enter('- [ ] a', 7)?.doc).toBe('- [ ] a\n- [ ] ');
    expect(enter('- [x] done', 10)?.doc).toBe('- [x] done\n- [ ] ');
  });

  it('puts the text after the caret into the new item', () => {
    const r = enter('- ab', 3);
    expect(r?.doc).toBe('- a\n- b');
    expect(r?.anchor).toBe(6);
  });

  it('ends the list on an empty item', () => {
    const r = enter('- a\n- ', 6);
    expect(r?.doc).toBe('- a\n');
    expect(r?.anchor).toBe(4);
    expect(enter('- [ ] ', 6)?.doc).toBe('');
    expect(enter('2. ', 3)?.doc).toBe('');
  });

  it('lifts an empty nested item one level before ending the list', () => {
    expect(enter('- a\n  - ', 8)?.doc).toBe('- a\n- ');
  });

  it('leaves other lines and the marker itself to the default Enter', () => {
    expect(enter('plain', 5)).toBeNull();
    expect(enter('- a', 1)).toBeNull();
    expect(enter('# h', 3)).toBeNull();
  });
});

describe('listIndent', () => {
  const tab = (doc: string, from: number, to: number, out = false) => {
    const e = listIndent(doc, from, to, out);
    return e ? applyEdit(doc, e).doc : null;
  };

  it('nests by the marker width', () => {
    expect(tab('- a\n- b', 4, 4)).toBe('- a\n  - b');
    expect(tab('1. a\n2. b', 5, 5)).toBe('1. a\n   2. b');
  });

  it('nests every selected item and ignores other lines', () => {
    expect(tab('- a\ntext\n- b', 0, 12)).toBe('  - a\ntext\n  - b');
  });

  it('lifts with Shift+Tab, never past the margin', () => {
    expect(tab('- a\n  - b', 4, 4, true)).toBe('- a\n- b');
    expect(tab('- a', 0, 0, true)).toBe('- a');
    expect(tab('\t- a', 1, 1, true)).toBe('- a');
  });

  it('is null away from lists, so Tab moves focus', () => {
    expect(listIndent('plain', 0, 0, false)).toBeNull();
    expect(listIndent('# h', 0, 0, true)).toBeNull();
  });
});

describe('markerBackspace', () => {
  const back = (doc: string, pos: number) => {
    const e = markerBackspace(doc, pos);
    return e ? applyEdit(doc, e).doc : null;
  };

  it('removes the marker when the caret sits right after it', () => {
    expect(back('- a', 2)).toBe('a');
    expect(back('  1. a', 5)).toBe('a');
    expect(back('- [ ] a', 6)).toBe('a');
    expect(back('x\n- ', 4)).toBe('x\n');
  });

  it('also unwraps a quote or heading marker', () => {
    expect(back('> q', 2)).toBe('q');
    expect(back('## t', 3)).toBe('t');
  });

  it('is null anywhere else', () => {
    expect(back('- a', 3)).toBeNull();
    expect(back('- a', 1)).toBeNull();
    expect(back('plain', 0)).toBeNull();
  });
});
