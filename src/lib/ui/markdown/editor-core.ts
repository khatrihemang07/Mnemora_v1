/**
 * The CodeMirror 6 editor behind MarkdownEditor. This module (and everything it imports from
 * CodeMirror) is only ever loaded through a dynamic import, so the editor stays out of the main
 * bundle. The behaviour itself lives in the plain helpers next to it; this file wires them to
 * CodeMirror.
 */
import { history, historyKeymap, insertNewline, standardKeymap } from '@codemirror/commands';
import { markdown } from '@codemirror/lang-markdown';
import { syntaxTree } from '@codemirror/language';
import {
  Annotation,
  EditorSelection,
  EditorState,
  type Extension,
  Prec,
  type TransactionSpec,
} from '@codemirror/state';
import { EditorView, keymap, placeholder, type ViewUpdate } from '@codemirror/view';
import { autoPair, pairBackspace } from './auto-pair';
import { checklistBoxes } from './checklist';
import { markdownDecorations } from './decorations';
import type { Edit } from './edit';
import { linkTemplate, makeLink, toggleBlock, toggleInline } from './format';
import { listEnter, listIndent, markerBackspace } from './list-keys';
import { htmlToMarkdown, isRichHtml } from './paste-html';
import { type EditorCommand, isPlainPasteShortcut, resolveEditorShortcut } from './shortcuts';
import {
  applySlash,
  canOpenSlash,
  filterSlash,
  type SlashItem,
  type SlashValues,
  slashQuery,
} from './slash';
import { markdownExtensions } from './syntax';
import { editorTheme } from './theme';

/** Marks changes that came from the host (the `value` prop), so they are not reported back. */
const external = Annotation.define<boolean>();

/** The open slash menu: what to list, which row is active, and where the caret is on screen. */
export interface SlashState {
  items: SlashItem[];
  index: number;
  /** Viewport coordinates of the caret line. */
  rect: { left: number; top: number; bottom: number };
}

export interface EditorOptions {
  parent: HTMLElement;
  doc: string;
  label: string;
  placeholder: string;
  spellcheck: boolean;
  /** The container shows focus itself, so the editor draws no ring. */
  bare: boolean;
  /** Draw Checklist items as clickable square boxes instead of faded `- [ ]` text. */
  checklist?: boolean;
  onchange: (text: string) => void;
  /** ⌘/Ctrl+Enter. */
  onfinish: () => void;
  /** Esc, with no slash menu open. Separate from `onfinish` so a caller can make Esc leave the editor. */
  onescape: () => void;
  onfocuschange: (focused: boolean) => void;
  onslash: (state: SlashState | null) => void;
  slashValues: () => SlashValues;
  /** ⌘/Ctrl-click on a link. */
  onopenlink: (href: string) => void;
}

export interface EditorController {
  setDoc: (text: string) => void;
  getDoc: () => string;
  focus: () => void;
  blur: () => void;
  run: (command: EditorCommand) => boolean;
  pickSlash: (id: string) => void;
  /** Turns spellcheck on or off in the open editor. */
  setSpellcheck: (on: boolean) => void;
  /** Tells assistive tech which menu option is active (null when the menu is closed). */
  setPopup: (listId: string, optionId: string | null) => void;
  destroy: () => void;
}

const SAFE_HREF = /^(https?:|mailto:|tel:)/i;
const isMacLike = () => /mac|iphone|ipad/i.test(navigator.platform || navigator.userAgent);

export function createEditor(opts: EditorOptions): EditorController {
  const mac = isMacLike();
  let slash: { pos: number; query: string; index: number } | null = null;
  let plainPaste = false;

  const doc = (v: EditorView) => v.state.doc.toString();

  function apply(view: EditorView, edit: Edit, userEvent = 'input'): void {
    const spec: TransactionSpec = { changes: edit.changes, userEvent, scrollIntoView: true };
    if (edit.selection) {
      // The selection is in post-change coordinates, which `EditorSelection` expects as well.
      spec.selection = EditorSelection.single(edit.selection.anchor, edit.selection.head);
    }
    view.dispatch(spec);
  }

  function run(view: EditorView, command: EditorCommand, fromBar = false): boolean {
    const text = doc(view);
    const { from, to } = view.state.selection.main;
    let edit: Edit | null;
    if (command.type === 'inline') edit = toggleInline(text, from, to, command.mark);
    else if (command.type === 'block') edit = toggleBlock(text, from, to, command.kind);
    else edit = makeLink(text, from, to) ?? (fromBar ? linkTemplate(from) : null);
    if (!edit) return false;
    apply(view, edit, 'input.format');
    return true;
  }

  // ---- slash menu ----

  function emitSlash(view: EditorView) {
    if (!slash) return opts.onslash(null);
    const items = filterSlash(slash.query);
    const caret = view.state.selection.main.head;
    const at = view.coordsAtPos(caret);
    if (items.length === 0 || !at) {
      slash = null;
      return opts.onslash(null);
    }
    slash.index = Math.min(slash.index, items.length - 1);
    opts.onslash({
      items,
      index: slash.index,
      rect: { left: at.left, top: at.top, bottom: at.bottom },
    });
  }

  function closeSlash(view: EditorView) {
    slash = null;
    emitSlash(view);
  }

  function chooseSlash(view: EditorView, item: SlashItem | undefined) {
    if (!slash || !item) return;
    const caret = view.state.selection.main.head;
    const edit = applySlash(doc(view), slash.pos, caret, item, opts.slashValues());
    slash = null;
    opts.onslash(null);
    apply(view, edit, 'input.slash');
    view.focus();
  }

  /** Opens, follows or closes the menu after the document or selection changed. */
  function trackSlash(update: ViewUpdate) {
    const view = update.view;
    const text = doc(view);
    const caret = view.state.selection.main.head;
    if (slash) {
      slash.pos = update.changes.mapPos(slash.pos, -1);
      const query = slashQuery(text, slash.pos, caret);
      if (query === null) slash = null;
      else {
        if (query !== slash.query) slash.index = 0;
        slash.query = query;
      }
    } else if (update.docChanged && update.transactions.some((t) => t.isUserEvent('input.type'))) {
      // A typed "/" opens the menu at a line start or after a space.
      update.changes.iterChanges((_fa, _ta, fromB, toB, inserted) => {
        if (toB - fromB === 1 && inserted.toString() === '/' && canOpenSlash(text, fromB)) {
          slash = { pos: fromB, query: '', index: 0 };
        }
      });
    }
    emitSlash(view);
  }

  // ---- keys ----

  const keys = keymap.of([
    {
      key: 'Escape',
      run: (v) => {
        if (slash) {
          closeSlash(v);
          return true;
        }
        opts.onescape();
        return true;
      },
    },
    {
      key: 'Mod-Enter',
      run: () => {
        opts.onfinish();
        return true;
      },
    },
    {
      key: 'ArrowDown',
      run: (v) => {
        if (!slash) return false;
        slash.index = (slash.index + 1) % Math.max(1, filterSlash(slash.query).length);
        emitSlash(v);
        return true;
      },
    },
    {
      key: 'ArrowUp',
      run: (v) => {
        if (!slash) return false;
        const n = Math.max(1, filterSlash(slash.query).length);
        slash.index = (slash.index - 1 + n) % n;
        emitSlash(v);
        return true;
      },
    },
    {
      key: 'Enter',
      run: (v) => {
        if (slash) {
          chooseSlash(v, filterSlash(slash.query)[slash.index]);
          return true;
        }
        const { from, to } = v.state.selection.main;
        const edit = from === to ? listEnter(doc(v), from) : null;
        if (edit) apply(v, edit, 'input.type');
        else insertNewline(v);
        return true;
      },
    },
    {
      key: 'Tab',
      run: (v) => {
        if (slash) {
          chooseSlash(v, filterSlash(slash.query)[slash.index]);
          return true;
        }
        const { from, to } = v.state.selection.main;
        const edit = listIndent(doc(v), from, to, false);
        if (edit) apply(v, edit);
        return edit !== null;
      },
    },
    {
      key: 'Shift-Tab',
      run: (v) => {
        const { from, to } = v.state.selection.main;
        const edit = listIndent(doc(v), from, to, true);
        if (edit) apply(v, edit);
        return edit !== null;
      },
    },
    {
      key: 'Backspace',
      run: (v) => {
        const { from, to } = v.state.selection.main;
        if (from !== to) return false;
        const edit = pairBackspace(doc(v), from) ?? markerBackspace(doc(v), from);
        if (edit) apply(v, edit, 'delete.backward');
        return edit !== null;
      },
    },
  ]);

  // ---- DOM events ----

  /** The link under a click, as a safe address (or null). */
  function linkAt(view: EditorView, pos: number): string | null {
    for (
      let n: ReturnType<typeof syntaxTree>['topNode'] | null = syntaxTree(view.state).resolveInner(
        pos,
        1,
      );
      n;
      n = n.parent
    ) {
      if (n.name === 'Link' || n.name === 'Autolink' || n.name === 'URL') {
        const url = n.name === 'URL' ? n : n.getChild('URL');
        const href = url ? view.state.sliceDoc(url.from, url.to) : '';
        return SAFE_HREF.test(href) ? href : null;
      }
    }
    return null;
  }

  const domEvents = EditorView.domEventHandlers({
    keydown(e, view) {
      if (isPlainPasteShortcut(e, mac)) {
        // ⌘⇧V: the next paste is plain text, whatever the clipboard holds.
        plainPaste = true;
        setTimeout(() => {
          plainPaste = false;
        }, 500);
        return false;
      }
      const command = resolveEditorShortcut(e, mac);
      if (!command) return false;
      // ⌘K with nothing selected falls through so the app palette opens.
      if (!run(view, command)) return false;
      e.preventDefault();
      e.stopPropagation();
      return true;
    },
    paste(e, view) {
      if (plainPaste) return false;
      const html = e.clipboardData?.getData('text/html') ?? '';
      if (!html || !isRichHtml(html)) return false;
      const md = htmlToMarkdown(html);
      if (!md) return false;
      e.preventDefault();
      view.dispatch(view.state.replaceSelection(md), {
        userEvent: 'input.paste',
        scrollIntoView: true,
      });
      return true;
    },
    mousedown(e, view) {
      if (!(mac ? e.metaKey : e.ctrlKey)) return false;
      const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
      const href = pos === null ? null : linkAt(view, pos);
      if (!href) return false;
      e.preventDefault();
      opts.onopenlink(href);
      return true;
    },
    blur() {
      opts.onfocuschange(false);
      return false;
    },
    focus() {
      opts.onfocuschange(true);
      return false;
    },
  });

  // Typed characters: auto-pairs. Not while an IME is composing.
  const input = EditorView.inputHandler.of((view, from, to, text) => {
    if (view.composing || text.length !== 1) return false;
    const edit = autoPair(doc(view), from, to, text);
    if (!edit) return false;
    apply(view, edit, 'input.type');
    return true;
  });

  const extensions: Extension[] = [
    history(),
    markdown({ extensions: markdownExtensions, addKeymap: false, completeHTMLTags: false }),
    markdownDecorations,
    ...(opts.checklist ? [checklistBoxes] : []),
    EditorView.lineWrapping,
    EditorView.contentAttributes.of({
      'aria-label': opts.label,
      spellcheck: String(opts.spellcheck),
      autocapitalize: 'sentences',
      ...(opts.bare ? { 'data-bare': 'true' } : {}),
    }),
    placeholder(opts.placeholder),
    editorTheme,
    Prec.highest(keys),
    keymap.of([...historyKeymap, ...standardKeymap]),
    domEvents,
    input,
    EditorView.updateListener.of((u) => {
      if (u.docChanged && !u.transactions.every((t) => t.annotation(external))) {
        opts.onchange(u.state.doc.toString());
      }
      if (u.docChanged || u.selectionSet) trackSlash(u);
      else if (slash && u.viewportChanged) emitSlash(u.view);
    }),
  ];

  const view = new EditorView({
    parent: opts.parent,
    state: EditorState.create({
      doc: opts.doc,
      selection: EditorSelection.cursor(opts.doc.length),
      extensions,
    }),
  });

  // Web fonts change line metrics after the first measure.
  void document.fonts?.ready.then(() => view.requestMeasure());

  return {
    setDoc(text) {
      if (text === doc(view)) return;
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: text },
        annotations: external.of(true),
      });
    },
    getDoc: () => doc(view),
    focus: () => view.focus(),
    blur: () => view.contentDOM.blur(),
    run: (command) => {
      view.focus();
      return run(view, command, true);
    },
    setSpellcheck: (on) => view.contentDOM.setAttribute('spellcheck', String(on)),
    setPopup(listId, optionId) {
      const el = view.contentDOM;
      if (optionId === null) {
        el.removeAttribute('aria-activedescendant');
        el.removeAttribute('aria-controls');
        return;
      }
      el.setAttribute('aria-controls', listId);
      el.setAttribute('aria-activedescendant', optionId);
    },
    pickSlash: (id) =>
      chooseSlash(
        view,
        filterSlash(slash?.query ?? '').find((i) => i.id === id),
      ),
    destroy: () => view.destroy(),
  };
}
