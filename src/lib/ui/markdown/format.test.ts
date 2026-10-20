import { describe, expect, it } from 'vitest';
import { applyEdit } from './edit';
import { blockKind, linkTemplate, makeLink, toggleBlock, toggleInline } from './format';

const inline = (doc: string, from: number, to: number, mark: Parameters<typeof toggleInline>[3]) =>
  applyEdit(doc, toggleInline(doc, from, to, mark));

describe('toggleInline', () => {
  it('wraps a selection and keeps the words selected', () => {
    const r = inline('say hello now', 4, 9, '**');
    expect(r.doc).toBe('say **hello** now');
    expect([r.anchor, r.head]).toEqual([6, 11]);
  });

  it('puts an empty pair around a caret', () => {
    const r = inline('ab', 1, 1, '`');
    expect(r.doc).toBe('a``b');
    expect(r.anchor).toBe(2);
  });

  it('unwraps when the marks sit just outside the selection', () => {
    const r = inline('a **b** c', 4, 5, '**');
    expect(r.doc).toBe('a b c');
    expect([r.anchor, r.head]).toEqual([2, 3]);
  });

  it('italic is not confused with bold, and both can stack', () => {
    expect(inline('**b**', 2, 3, '*').doc).toBe('***b***');
    expect(inline('***b***', 3, 4, '*').doc).toBe('**b**');
    expect(inline('***b***', 3, 4, '**').doc).toBe('*b*');
  });

  it('keeps marks tight around the words when the selection has spaces', () => {
    expect(inline('a hello b', 1, 8, '~~').doc).toBe('a ~~hello~~ b');
  });

  it('does nothing for a whitespace-only selection', () => {
    expect(inline('a   b', 1, 4, '**').doc).toBe('a   b');
  });

  it('supports strikethrough and highlight', () => {
    expect(inline('x', 0, 1, '~~').doc).toBe('~~x~~');
    expect(inline('x', 0, 1, '==').doc).toBe('==x==');
  });
});

const block = (doc: string, from: number, to: number, kind: Parameters<typeof toggleBlock>[3]) =>
  applyEdit(doc, toggleBlock(doc, from, to, kind)).doc;

describe('toggleBlock', () => {
  it('prefixes the caret line', () => {
    expect(block('hello', 2, 2, 'bullet')).toBe('- hello');
    expect(block('hello', 2, 2, 'task')).toBe('- [ ] hello');
    expect(block('hello', 0, 0, 'quote')).toBe('> hello');
  });

  it('works on an empty line', () => {
    expect(block('', 0, 0, 'h1')).toBe('# ');
  });

  it('replaces another block prefix', () => {
    expect(block('# title', 0, 0, 'h2')).toBe('## title');
    expect(block('- a', 0, 0, 'numbered')).toBe('1. a');
    expect(block('1. a', 0, 0, 'task')).toBe('- [ ] a');
  });

  it('toggles off when every line already has it', () => {
    expect(block('- a\n- b', 0, 7, 'bullet')).toBe('a\nb');
    expect(block('## t', 0, 0, 'h2')).toBe('t');
  });

  it('numbers several lines and skips blank ones', () => {
    expect(block('a\n\nb\nc', 0, 6, 'numbered')).toBe('1. a\n\n2. b\n3. c');
  });

  it('keeps a list indent but not for headings', () => {
    expect(block('  - a', 0, 0, 'numbered')).toBe('  1. a');
    expect(block('  a', 0, 0, 'h1')).toBe('# a');
  });

  it('a selection ending at a line start leaves that line alone', () => {
    expect(block('a\nb', 0, 2, 'bullet')).toBe('- a\nb');
  });
});

describe('blockKind', () => {
  it('names the prefix, and ignores headings past 3', () => {
    expect(blockKind('# a')).toBe('h1');
    expect(blockKind('### a')).toBe('h3');
    expect(blockKind('#### a')).toBeNull();
    expect(blockKind('> a')).toBe('quote');
    expect(blockKind('- [x] a')).toBe('task');
    expect(blockKind('* a')).toBe('bullet');
    expect(blockKind('12. a')).toBe('numbered');
    expect(blockKind('plain')).toBeNull();
  });
});

describe('makeLink', () => {
  it('needs a selection', () => {
    expect(makeLink('abc', 1, 1)).toBeNull();
    expect(makeLink('a  c', 1, 3)).toBeNull();
  });

  it('wraps text and selects the url placeholder', () => {
    const e = makeLink('see docs now', 4, 8);
    expect(e).not.toBeNull();
    if (!e) return;
    const r = applyEdit('see docs now', e);
    expect(r.doc).toBe('see [docs](url) now');
    expect(r.doc.slice(r.anchor, r.head)).toBe('url');
  });

  it('a pasted-looking url becomes the destination with the caret in the label', () => {
    const e = makeLink('https://x.y/z', 0, 13);
    if (!e) throw new Error('expected an edit');
    const r = applyEdit('https://x.y/z', e);
    expect(r.doc).toBe('[](https://x.y/z)');
    expect(r.anchor).toBe(1);
  });
});

describe('linkTemplate', () => {
  it('inserts a link skeleton with the label selected', () => {
    const r = applyEdit('ab', linkTemplate(1));
    expect(r.doc).toBe('a[text](url)b');
    expect(r.doc.slice(r.anchor, r.head)).toBe('text');
  });
});

describe('toggleBlock caret', () => {
  const at = (doc: string, pos: number, kind: Parameters<typeof toggleBlock>[3]) => {
    const r = applyEdit(doc, toggleBlock(doc, pos, pos, kind));
    return [r.doc, r.head] as const;
  };

  it('lands after the marker on an empty line', () => {
    expect(at('', 0, 'task')).toEqual(['- [ ] ', 6]);
    expect(at('', 0, 'bullet')).toEqual(['- ', 2]);
    expect(at('', 0, 'quote')).toEqual(['> ', 2]);
    expect(at('', 0, 'h2')).toEqual(['## ', 3]);
  });

  it('keeps the caret in the text when the line has words', () => {
    expect(at('hello', 0, 'task')).toEqual(['- [ ] hello', 6]);
    expect(at('hello', 3, 'bullet')).toEqual(['- hello', 5]);
  });

  it('moves the caret back when the marker is removed or swapped', () => {
    expect(at('- hi', 4, 'bullet')).toEqual(['hi', 2]);
    expect(at('# hi', 1, 'h2')).toEqual(['## hi', 3]);
  });
});
