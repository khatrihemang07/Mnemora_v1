import { describe, expect, it } from 'vitest';
import { type Block, type Inline, parseMarkdown } from './ast';

const text = (s: string): Inline => ({ t: 'text', s });
const para = (...c: Inline[]): Block => ({ t: 'p', c });

describe('parseMarkdown', () => {
  it('keeps plain text and single newlines as one paragraph', () => {
    expect(parseMarkdown('one\ntwo\n\nthree')).toEqual([
      para(text('one\ntwo')),
      para(text('three')),
    ]);
  });

  it('renders headings 1-3 without their marks and leaves deeper ones as text', () => {
    expect(parseMarkdown('# A\n## B **x**\n### C')).toEqual([
      { t: 'h', level: 1, c: [text('A')] },
      { t: 'h', level: 2, c: [text('B '), { t: 'strong', c: [text('x')] }] },
      { t: 'h', level: 3, c: [text('C')] },
    ]);
    expect(parseMarkdown('#### D')).toEqual([para(text('#### D'))]);
  });

  it('supports bold, italic, strikethrough, highlight and inline code', () => {
    expect(parseMarkdown('**b** *i* ~~s~~ ==h== `c`')).toEqual([
      para(
        { t: 'strong', c: [text('b')] },
        text(' '),
        { t: 'em', c: [text('i')] },
        text(' '),
        { t: 'del', c: [text('s')] },
        text(' '),
        { t: 'mark', c: [text('h')] },
        text(' '),
        { t: 'code', s: 'c' },
      ),
    ]);
  });

  it('does not treat spaced == as a highlight', () => {
    expect(parseMarkdown('a == b == c')).toEqual([para(text('a == b == c'))]);
  });

  it('links: markdown, bare and angle forms open; unsafe schemes stay text', () => {
    expect(parseMarkdown('[a](https://x.y) https://z.com/p.')).toEqual([
      para(
        { t: 'link', href: 'https://x.y', c: [text('a')] },
        text(' '),
        { t: 'link', href: 'https://z.com/p', c: [text('https://z.com/p')] },
        text('.'),
      ),
    ]);
    expect(parseMarkdown('[a](javascript:alert(1))')).toEqual([
      para(text('[a](javascript:alert(1))')),
    ]);
  });

  it('quotes drop their prefix on every line', () => {
    expect(parseMarkdown('> a\n> b')).toEqual([{ t: 'quote', c: [para(text('a\nb'))] }]);
  });

  it('lists: bullet, numbered, checklist with checked state and line', () => {
    expect(parseMarkdown('- a\n- b')).toEqual([
      {
        t: 'list',
        ordered: false,
        start: 1,
        items: [
          { task: null, c: [para(text('a'))] },
          { task: null, c: [para(text('b'))] },
        ],
      },
    ]);
    expect(parseMarkdown('3. x\n4. y')[0]).toMatchObject({ ordered: true, start: 3 });
    expect(parseMarkdown('hi\n\n- [ ] a\n- [x] b')[1]).toMatchObject({
      items: [
        { task: { checked: false, line: 2 }, c: [para(text('a'))] },
        { task: { checked: true, line: 3 }, c: [para(text('b'))] },
      ],
    });
  });

  it('nested list items keep their own list', () => {
    const [list] = parseMarkdown('- a\n  - b');
    expect(list).toMatchObject({ t: 'list' });
    const item = (list as Extract<Block, { t: 'list' }>).items[0];
    expect(item?.c[1]).toMatchObject({ t: 'list' });
  });

  it('divider and fenced code', () => {
    expect(parseMarkdown('a\n\n---\n\n```js\nlet x = 1\n```')).toEqual([
      para(text('a')),
      { t: 'hr' },
      { t: 'code', s: 'let x = 1' },
    ]);
  });

  it('keeps tables, html and images as plain text', () => {
    expect(parseMarkdown('| a | b |\n|---|---|\n| 1 | 2 |')).toEqual([
      para(text('| a | b |\n|---|---|\n| 1 | 2 |')),
    ]);
    expect(parseMarkdown('<div>x</div>')).toEqual([para(text('<div>x</div>'))]);
    expect(parseMarkdown('![a](u)')).toEqual([para(text('![a](u)'))]);
  });

  it('produces data only: no html field exists to inject', () => {
    expect(JSON.stringify(parseMarkdown('<script>x</script> **a**'))).not.toContain('"html"');
  });
});
