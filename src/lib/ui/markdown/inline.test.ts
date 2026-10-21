import { describe, expect, it } from 'vitest';
import { parseInlineMarkdown } from './inline';

const text = (s: string) => ({ t: 'text' as const, s });

describe('parseInlineMarkdown', () => {
  it('marks up bold, italic, code and links', () => {
    expect(parseInlineMarkdown('**b** *i* `c` [a](https://x.y)')).toEqual([
      { t: 'strong', c: [text('b')] },
      text(' '),
      { t: 'em', c: [text('i')] },
      text(' '),
      { t: 'code', s: 'c' },
      text(' '),
      { t: 'link', href: 'https://x.y', c: [text('a')] },
    ]);
  });

  it('links bare URLs and leaves unsafe schemes as text', () => {
    expect(parseInlineMarkdown('see https://z.com/p now')).toEqual([
      text('see '),
      { t: 'link', href: 'https://z.com/p', c: [text('https://z.com/p')] },
      text(' now'),
    ]);
    expect(parseInlineMarkdown('[x](javascript:alert(1))')).toEqual([
      text('[x](javascript:alert(1))'),
    ]);
  });

  it('shows block syntax as the text it is', () => {
    for (const line of [
      '# Heading',
      '## Two',
      '> quote',
      '- item',
      '* item',
      '+ item',
      '1. first',
      '2) second',
      '- [ ] box',
      '```fence```',
      '---',
      '***',
      '<div>',
    ]) {
      expect(parseInlineMarkdown(line), line).toEqual([text(line)]);
    }
  });

  it('does not read a link reference definition but still links its URL', () => {
    expect(parseInlineMarkdown('[ref]: https://x.y')).toEqual([
      text('[ref]: '),
      { t: 'link', href: 'https://x.y', c: [text('https://x.y')] },
    ]);
  });

  it('still marks up inline syntax after a block marker', () => {
    expect(parseInlineMarkdown('# **big**')).toEqual([
      text('# '),
      { t: 'strong', c: [text('big')] },
    ]);
    expect(parseInlineMarkdown('1. a `b`')).toEqual([text('1. a '), { t: 'code', s: 'b' }]);
  });

  it('keeps strikethrough and highlight marks as text', () => {
    expect(parseInlineMarkdown('a ~~b~~ ==c==')).toEqual([text('a ~~b~~ ==c==')]);
    expect(parseInlineMarkdown('**~~x~~**')).toEqual([{ t: 'strong', c: [text('~~x~~')] }]);
  });

  it('reads several lines, keeping the breaks and block syntax on later lines as text', () => {
    expect(parseInlineMarkdown('one\n- two **b**')).toEqual([
      text('one\n- two '),
      { t: 'strong', c: [text('b')] },
    ]);
    expect(parseInlineMarkdown('a\n\nb')).toEqual([text('a\n\nb')]);
  });

  it('keeps ordinary text, including hash tags and stars, as it is', () => {
    expect(parseInlineMarkdown('Call #home about 5 * 3')).toEqual([text('Call #home about 5 * 3')]);
    expect(parseInlineMarkdown('')).toEqual([]);
  });
});
