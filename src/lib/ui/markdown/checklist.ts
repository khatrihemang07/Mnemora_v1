import { ensureSyntaxTree, syntaxTree } from '@codemirror/language';
import { type EditorState, type Extension, type Range, StateField } from '@codemirror/state';
import { Decoration, type DecorationSet, EditorView, WidgetType } from '@codemirror/view';
import { toggleTaskLine } from './tasks';

const TICK_SVG =
  '<svg viewBox="0 0 16 16" width="11" height="11" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3.2 8.6 6.6 12 12.8 4.6"/></svg>';

/** A Checklist item's square box, drawn in place of its `- [ ]` / `- [x]` marker. */
class BoxWidget extends WidgetType {
  constructor(private readonly checked: boolean) {
    super();
  }

  override eq(other: BoxWidget): boolean {
    return other.checked === this.checked;
  }

  override toDOM(view: EditorView): HTMLElement {
    const box = document.createElement('span');
    box.className = 'md-cb cm-md-cb';
    box.setAttribute('role', 'checkbox');
    box.setAttribute('aria-checked', String(this.checked));
    if (this.checked) {
      box.dataset.checked = '';
      box.innerHTML = TICK_SVG;
    }
    // The editor keeps focus and its caret; the box only flips the line.
    box.addEventListener('mousedown', (e) => {
      e.preventDefault();
      toggleAt(view, view.posAtDOM(box));
    });
    return box;
  }

  override ignoreEvent(): boolean {
    return true;
  }
}

/** Flips the Checklist item on the line holding `pos`; the text change is a normal edit. */
function toggleAt(view: EditorView, pos: number): void {
  const line = view.state.doc.lineAt(pos);
  const next = toggleTaskLine(view.state.doc.toString(), line.number - 1);
  if (next === null) return;
  const text = next.split('\n')[line.number - 1] ?? line.text;
  view.dispatch({
    changes: { from: line.from, to: line.to, insert: text },
    userEvent: 'input.tick',
  });
}

function build(state: EditorState): DecorationSet {
  const tree = ensureSyntaxTree(state, state.doc.length, 100) ?? syntaxTree(state);
  const out: Range<Decoration>[] = [];
  tree.iterate({
    enter(node) {
      if (node.name !== 'TaskMarker') return;
      const mark = node.node.parent?.parent?.getChild('ListMark');
      if (!mark || mark.to > node.from) return;
      const checked = /x/i.test(state.sliceDoc(node.from, node.to));
      out.push(Decoration.replace({ widget: new BoxWidget(checked) }).range(mark.from, node.to));
    },
  });
  return Decoration.set(out, true);
}

const boxes = StateField.define<DecorationSet>({
  create: build,
  update: (value, tr) =>
    tr.docChanged || syntaxTree(tr.state) !== syntaxTree(tr.startState) ? build(tr.state) : value,
  provide: (field) => [
    EditorView.decorations.from(field),
    EditorView.atomicRanges.of((view) => view.state.field(field)),
  ],
});

/**
 * Draws each `- [ ]` / `- [x]` line start as a clickable square box (Todo's notes). A click flips
 * the line like typing would; the box and its dash take the place of the marker, which Backspace
 * removes as one piece. Left out, the marker stays as faded text (the Journal's editor).
 */
export const checklistBoxes: Extension = boxes;
