import { describe, expect, it } from 'vitest';
import { htmlToMarkdown, isRichHtml } from './paste-html';

describe('htmlToMarkdown', () => {
  it('converts inline formatting', () => {
    expect(
      htmlToMarkdown(
        '<p>a <strong>b</strong> <em>c</em> <del>d</del> <mark>e</mark> <code>f</code></p>',
      ),
    ).toBe('a **b** *c* ~~d~~ ==e== `f`');
  });

  it('keeps spaces outside the marks', () => {
    expect(htmlToMarkdown('<p>one<b> two </b>three</p>')).toBe('one **two** three');
  });

  it('converts links, collapsing a link that shows its own address', () => {
    expect(htmlToMarkdown('<a href="https://x.y/a">docs</a>')).toBe('[docs](https://x.y/a)');
    expect(htmlToMarkdown('<a href="https://x.y">https://x.y</a>')).toBe('https://x.y');
    expect(htmlToMarkdown('<a href="javascript:alert(1)">bad</a>')).toBe('bad');
  });

  it('converts headings, capping at level 3', () => {
    expect(htmlToMarkdown('<h1>A</h1><h2>B</h2><h5>C</h5><p>t</p>')).toBe(
      '# A\n\n## B\n\n### C\n\nt',
    );
  });

  it('converts bullet, numbered and nested lists', () => {
    expect(htmlToMarkdown('<ul><li>a</li><li>b<ul><li>c</li></ul></li></ul>')).toBe(
      '- a\n- b\n  - c',
    );
    expect(htmlToMarkdown('<ol start="3"><li>x</li><li>y</li></ol>')).toBe('3. x\n4. y');
  });

  it('converts checkbox items', () => {
    expect(
      htmlToMarkdown(
        '<ul><li><input type="checkbox" checked> done</li><li><input type="checkbox"> todo</li></ul>',
      ),
    ).toBe('- [x] done\n- [ ] todo');
  });

  it('converts quotes, code blocks and dividers', () => {
    expect(htmlToMarkdown('<blockquote><p>a</p><p>b</p></blockquote>')).toBe('> a\n>\n> b');
    expect(htmlToMarkdown('<pre><code>x = 1\ny = 2</code></pre>')).toBe('```\nx = 1\ny = 2\n```');
    expect(htmlToMarkdown('<p>a</p><hr><p>b</p>')).toBe('a\n\n---\n\nb');
  });

  it('separates paragraphs with a blank line and divs with a newline', () => {
    expect(htmlToMarkdown('<p>a</p><p>b</p>')).toBe('a\n\nb');
    expect(htmlToMarkdown('<div>a</div><div>b</div>')).toBe('a\nb');
    expect(htmlToMarkdown('a<br>b')).toBe('a\nb');
  });

  it('decodes entities and collapses source whitespace', () => {
    expect(htmlToMarkdown('<p>Tom &amp; Jerry\n   &lt;3 &#233;&nbsp;x</p>')).toBe(
      'Tom & Jerry <3 é x',
    );
  });

  it('drops scripts, styles and comments, and never passes tags through', () => {
    const out = htmlToMarkdown(
      '<style>p{color:red}</style><!--StartFragment--><p>hi<script>alert(1)</script></p><p onclick="x()"><span>ok</span></p>',
    );
    expect(out).toBe('hi\n\nok');
  });

  it('reads bold and italic from inline styles, and ignores the Docs wrapper', () => {
    expect(
      htmlToMarkdown('<b style="font-weight:normal"><span style="font-weight:700">x</span> y</b>'),
    ).toBe('**x** y');
    expect(htmlToMarkdown('<span style="font-style:italic">i</span>')).toBe('*i*');
  });

  it('survives malformed markup', () => {
    expect(htmlToMarkdown('<p>a <b>b</p><p>c')).toBe('a **b**\n\nc');
    expect(htmlToMarkdown('1 < 2 and <')).toBe('1 < 2 and <');
  });
});

describe('isRichHtml', () => {
  it('is true only for markup that carries structure', () => {
    expect(isRichHtml('<p><b>x</b></p>')).toBe(true);
    expect(isRichHtml('<ul><li>a</li></ul>')).toBe(true);
    expect(isRichHtml('<div><span style="color:red">plain</span></div>')).toBe(false);
    expect(isRichHtml('plain')).toBe(false);
  });
});
