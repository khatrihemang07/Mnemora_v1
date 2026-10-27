import { EditorView } from '@codemirror/view';

/**
 * Mnemora's editor theme. Everything reads design tokens (CSS variables); the type matches the read
 * view (shared `.md-*` classes in markdown.css), so only the faded marks differ.
 */
export const editorTheme = EditorView.theme({
  '&': {
    color: 'var(--ink)',
    backgroundColor: 'transparent',
    fontSize: '1rem',
  },
  '&.cm-focused': { outline: 'none' },
  '.cm-scroller': {
    fontFamily: 'var(--md-font, var(--f-display))',
    lineHeight: '1.65',
    overflowX: 'hidden',
    overflowY: 'auto',
    maxHeight: 'var(--md-max-height, none)',
  },
  '.cm-content': {
    padding: '0',
    caretColor: 'var(--accent)',
    overflowWrap: 'anywhere',
  },
  '.cm-line': { padding: '0' },
  '.cm-placeholder': { color: 'var(--muted)' },

  '.cm-md-mark': { color: 'var(--faint)' },
  '.cm-md-strong': { fontWeight: '700' },
  '.cm-md-em': { fontStyle: 'italic' },
  '.cm-md-del': { textDecoration: 'line-through' },
  '.md-link': { cursor: 'text' },

  // A quote shows its ">" instead of the bar, and is not italic while it is being typed.
  '.cm-md-quote': { '--md-quote-bar': 'transparent', fontStyle: 'normal' },

  '.cm-md-code': {
    fontFamily: 'var(--f-mono)',
    fontSize: '0.86em',
    background: 'var(--surface-2)',
    padding: '0 0.8em',
    borderInline: '1px solid var(--line)',
  },
  '.cm-md-code-first': {
    paddingTop: '0.6em',
    borderTop: '1px solid var(--line)',
    borderStartStartRadius: 'var(--r1)',
    borderStartEndRadius: 'var(--r1)',
  },
  '.cm-md-code-last': {
    paddingBottom: '0.6em',
    boxShadow: 'inset 0 -1px 0 var(--line)',
    borderEndStartRadius: 'var(--r1)',
    borderEndEndRadius: 'var(--r1)',
  },

  // Rhythm of the read view: 0.55em between blocks.
  '.cm-md-gap': { paddingBottom: '0.55em' },
  '.cm-md-gap.cm-md-code-last': {
    paddingBottom: '0.6em',
    borderBottom: '0.55em solid transparent',
    backgroundClip: 'padding-box',
  },
  '.cm-md-blank': { lineHeight: '0.55em' },
});
