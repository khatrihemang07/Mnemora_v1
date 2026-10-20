import { Autolink, type MarkdownConfig, parser, Strikethrough, TaskList } from '@lezer/markdown';

const HIGHLIGHT_DELIM = { resolve: 'Highlight', mark: 'HighlightMark' };
const PUNCTUATION = /[\p{P}\p{S}]/u;
const EQUALS = 61;

/** `==highlight==`, shaped like the Strikethrough extension. */
const Highlight: MarkdownConfig = {
  defineNodes: [{ name: 'Highlight' }, { name: 'HighlightMark' }],
  parseInline: [
    {
      name: 'Highlight',
      parse(cx, next, pos) {
        if (next !== EQUALS || cx.char(pos + 1) !== EQUALS || cx.char(pos + 2) === EQUALS)
          return -1;
        const before = cx.slice(pos - 1, pos);
        const after = cx.slice(pos + 2, pos + 3);
        const spaceBefore = /\s|^$/.test(before);
        const spaceAfter = /\s|^$/.test(after);
        const punctBefore = PUNCTUATION.test(before);
        const punctAfter = PUNCTUATION.test(after);
        return cx.addDelimiter(
          HIGHLIGHT_DELIM,
          pos,
          pos + 2,
          !spaceAfter && (!punctAfter || spaceBefore || punctBefore),
          !spaceBefore && (!punctBefore || spaceAfter || punctAfter),
        );
      },
      after: 'Emphasis',
    },
  ],
};

/**
 * The Markdown dialect the Journal supports, shared by the editor and the read view so both see the
 * same tree. No tables: they stay plain text.
 */
export const markdownExtensions: MarkdownConfig[] = [Strikethrough, TaskList, Autolink, Highlight];

export const markdownParser = parser.configure(markdownExtensions);
