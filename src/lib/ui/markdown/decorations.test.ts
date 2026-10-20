import { markdown } from '@codemirror/lang-markdown';
import { EditorState } from '@codemirror/state';
import { describe, expect, it } from 'vitest';
import { markdownDecorations } from './decorations';
import { markdownExtensions } from './syntax';

/** Class names applied per line (by 1-based line number) and to inline ranges (as "text:class"). */
function decorate(doc: string) {
  const state = EditorState.create({
    doc,
    extensions: [
      markdown({ extensions: markdownExtensions, addKeymap: false }),
      markdownDecorations,
    ],
  });
  const lines = new Map<number, string>();
  const spans: string[] = [];
  const set = state.field(markdownDecorations);
  set.between(0, state.doc.length, (from, to, deco) => {
    const cls = (deco.spec as { class?: string }).class ?? '';
    if (from === to) lines.set(state.doc.lineAt(from).number, cls);
    else spans.push(`${state.sliceDoc(from, to)}:${cls}`);
  });
  return { lines, spans };
}

describe('markdown decorations', () => {
  it('fades every syntax mark and styles the text between them', () => {
    const { spans } = decorate('a **b** ==c== `d` ~~e~~');
    expect(spans).toEqual(
      expect.arrayContaining([
        '**b**:cm-md-strong',
        '**:cm-md-mark',
        '==c==:md-mark',
        '==:cm-md-mark',
        '`d`:md-code',
        '`:cm-md-mark',
        '~~e~~:cm-md-del',
        '~~:cm-md-mark',
      ]),
    );
  });

  it('marks headings 1-3 with the read view heading classes, and leaves deeper ones plain', () => {
    const { lines } = decorate('# a\n\n## b\n\n### c\n\n#### d');
    expect(lines.get(1)).toContain('md-hd md-h1');
    expect(lines.get(3)).toContain('md-h2');
    expect(lines.get(5)).toContain('md-h3');
    expect(lines.get(7) ?? '').not.toContain('md-hd');
  });

  it('styles links: label as a link, the address faded; bare urls as links', () => {
    const { spans } = decorate('[a](https://x.y) https://z.com');
    expect(spans).toEqual(
      expect.arrayContaining(['a:md-link', 'https://x.y:cm-md-mark', 'https://z.com:md-link']),
    );
  });

  it('gives the read view rhythm: gap lines and short blank lines', () => {
    const { lines } = decorate('# t\ntext\n\nnext\n\n');
    expect(lines.get(1)).toContain('cm-md-gap'); // heading directly followed by text
    expect(lines.get(2) ?? '').not.toContain('cm-md-gap'); // a blank line follows instead
    expect(lines.get(3)).toContain('cm-md-blank');
    expect(lines.get(5) ?? '').not.toContain('cm-md-blank'); // trailing blank lines stay full height
  });

  it('styles quote and code lines', () => {
    const { lines } = decorate('> q\n> r\n\n```\nx\n\ny\n```');
    expect(lines.get(1)).toContain('md-q');
    expect(lines.get(2)).toContain('cm-md-quote');
    expect(lines.get(4)).toContain('cm-md-code-first');
    expect(lines.get(8)).toContain('cm-md-code-last');
    expect(lines.get(6)).toContain('cm-md-code');
    expect(lines.get(6)).not.toContain('cm-md-blank');
  });
});
