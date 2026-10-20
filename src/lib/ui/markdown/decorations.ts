import { ensureSyntaxTree, syntaxTree } from '@codemirror/language';
import { type EditorState, type Range, StateField } from '@codemirror/state';
import { Decoration, type DecorationSet, EditorView } from '@codemirror/view';

/** Syntax marks that stay visible but faded while editing (M2). */
const MARKS = new Set([
  'EmphasisMark',
  'StrikethroughMark',
  'HighlightMark',
  'CodeMark',
  'HeaderMark',
  'QuoteMark',
  'ListMark',
  'TaskMarker',
  'LinkMark',
  'LinkTitle',
  'CodeInfo',
]);

/** Inline spans: the class that styles their text, shared with the read view where it exists. */
const SPANS: Record<string, string> = {
  StrongEmphasis: 'cm-md-strong',
  Emphasis: 'cm-md-em',
  Strikethrough: 'cm-md-del',
  Highlight: 'md-mark',
  InlineCode: 'md-code',
};

const fade = Decoration.mark({ class: 'cm-md-mark' });
const span = (cls: string) => Decoration.mark({ class: cls });
const spans = Object.fromEntries(Object.entries(SPANS).map(([k, v]) => [k, span(v)]));
const linkText = span('md-link');

type Classes = Map<number, Set<string>>;

function addLineClass(classes: Classes, lineFrom: number, cls: string) {
  const set = classes.get(lineFrom) ?? new Set<string>();
  set.add(cls);
  classes.set(lineFrom, set);
}

/** Adds `cls` to every line of `from..to`. */
function addLines(state: EditorState, classes: Classes, from: number, to: number, cls: string) {
  const last = state.doc.lineAt(to).number;
  for (let n = state.doc.lineAt(from).number; n <= last; n++) {
    addLineClass(classes, state.doc.line(n).from, cls);
  }
}

/**
 * Styles the Markdown: faded marks, inline styles, heading, quote and code lines, and the vertical
 * rhythm of the read view (a gap between blocks, a short blank line), so writing and reading
 * line up.
 */
function build(state: EditorState): DecorationSet {
  const tree = ensureSyntaxTree(state, state.doc.length, 100) ?? syntaxTree(state);
  const out: Range<Decoration>[] = [];
  const classes: Classes = new Map();
  const fences: { from: number; to: number }[] = [];

  tree.iterate({
    enter(node) {
      const name = node.name;
      if (MARKS.has(name)) {
        out.push(fade.range(node.from, node.to));
        return;
      }
      const deco = spans[name];
      if (deco) {
        out.push(deco.range(node.from, node.to));
      } else if (name === 'URL') {
        const parent = node.node.parent?.name;
        if (parent === 'Link' || parent === 'Image') out.push(fade.range(node.from, node.to));
        else out.push(linkText.range(node.from, node.to));
      } else if (name === 'Link') {
        const marks = node.node.getChildren('LinkMark');
        const open = marks[0];
        const close = marks[1];
        if (open && close && node.node.getChild('URL') && close.from > open.to) {
          out.push(linkText.range(open.to, close.from));
        }
      } else if (name === 'ATXHeading1' || name === 'ATXHeading2' || name === 'ATXHeading3') {
        addLineClass(classes, state.doc.lineAt(node.from).from, `md-hd md-h${name.slice(-1)}`);
      } else if (name === 'Blockquote') {
        addLines(state, classes, node.from, node.to, 'md-q cm-md-quote');
      } else if (name === 'FencedCode') {
        fences.push({ from: node.from, to: node.to });
        addLines(state, classes, node.from, node.to, 'cm-md-code');
        addLineClass(classes, state.doc.lineAt(node.from).from, 'cm-md-code-first');
        addLineClass(classes, state.doc.lineAt(node.to).from, 'cm-md-code-last');
      }
    },
  });

  // Blocks sit 0.55em apart in the read view: pad the last line of a block that is followed
  // directly by another one (a blank line between them provides the gap itself).
  const blocks: { from: number; to: number }[] = [];
  for (let n = tree.topNode.firstChild; n; n = n.nextSibling)
    blocks.push({ from: n.from, to: n.to });
  blocks.forEach((b, i) => {
    const next = blocks[i + 1];
    if (!next) return;
    const lastLine = state.doc.lineAt(b.to);
    if (state.doc.lineAt(next.from).number === lastLine.number + 1) {
      addLineClass(classes, lastLine.from, 'cm-md-gap');
    }
  });
  // Blank lines between blocks are as tall as the gap; trailing ones stay full height.
  let lastText = 0;
  for (let n = state.doc.lines; n >= 1; n--) {
    if (state.doc.line(n).text.trim() !== '') {
      lastText = n;
      break;
    }
  }
  for (let n = 1; n < lastText; n++) {
    const line = state.doc.line(n);
    const inFence = fences.some((f) => line.from >= f.from && line.from <= f.to);
    if (line.text.trim() === '' && !inFence) addLineClass(classes, line.from, 'cm-md-blank');
  }

  for (const [from, set] of classes) {
    out.push(Decoration.line({ class: [...set].join(' ') }).range(from));
  }
  return Decoration.set(out, true);
}

export const markdownDecorations = StateField.define<DecorationSet>({
  create: build,
  update: (value, tr) =>
    tr.docChanged || syntaxTree(tr.state) !== syntaxTree(tr.startState) ? build(tr.state) : value,
  provide: (field) => EditorView.decorations.from(field),
});
