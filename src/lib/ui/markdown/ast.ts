import { markdownParser } from './syntax';

/** Lezer's tree node, named through the parser (the tree package is not a direct dependency). */
type SyntaxNode = ReturnType<typeof markdownParser.parse>['topNode'];

export type Inline =
  | { t: 'text'; s: string }
  | { t: 'code'; s: string }
  | { t: 'strong'; c: Inline[] }
  | { t: 'em'; c: Inline[] }
  | { t: 'del'; c: Inline[] }
  | { t: 'mark'; c: Inline[] }
  | { t: 'link'; href: string; c: Inline[] };

export interface ListItem {
  /** Set for `- [ ]` / `- [x]` items; `line` is the item's 0-based line, for ticking later. */
  task: { checked: boolean; line: number } | null;
  c: Block[];
}

export type Block =
  | { t: 'p'; c: Inline[] }
  | { t: 'h'; level: 1 | 2 | 3; c: Inline[] }
  | { t: 'quote'; c: Block[] }
  | { t: 'hr' }
  | { t: 'code'; s: string }
  | { t: 'list'; ordered: boolean; start: number; items: ListItem[] };

/** Only these links are made clickable; anything else (`javascript:`) stays plain text. */
const SAFE_HREF = /^(https?:|mailto:|tel:)/i;

/** Syntax marks the read view hides. */
const HIDDEN = new Set([
  'EmphasisMark',
  'StrikethroughMark',
  'HighlightMark',
  'CodeMark',
  'HeaderMark',
  'TaskMarker',
  'ListMark',
]);

const INLINE_WRAPPERS: Record<string, 'strong' | 'em' | 'del' | 'mark'> = {
  StrongEmphasis: 'strong',
  Emphasis: 'em',
  Strikethrough: 'del',
  Highlight: 'mark',
};

/** Blocks the editor does not style: shown exactly as typed so nothing is lost. */
const RAW_BLOCKS = new Set([
  'CodeBlock',
  'HTMLBlock',
  'SetextHeading1',
  'SetextHeading2',
  'LinkReference',
  'CommentBlock',
  'ProcessingInstructionBlock',
  'ATXHeading4',
  'ATXHeading5',
  'ATXHeading6',
]);

/** Parses Markdown into plain data. Never produces HTML: the view renders each node itself. */
export function parseMarkdown(src: string): Block[] {
  const tree = markdownParser.parse(src);
  return new Builder(src).blocks(tree.topNode);
}

class Builder {
  constructor(private readonly src: string) {}

  blocks(parent: SyntaxNode, nested = false): Block[] {
    const out: Block[] = [];
    for (let n = parent.firstChild; n; n = n.nextSibling) {
      const block = this.block(n, nested);
      if (block) out.push(block);
    }
    return out;
  }

  private block(n: SyntaxNode, nested: boolean): Block | null {
    const name = n.name;
    if (name === 'Paragraph') return { t: 'p', c: this.inlines(n, n.from, n.to, nested) };
    const heading = /^ATXHeading([123])$/.exec(name);
    if (heading) {
      const level = Number(heading[1]) as 1 | 2 | 3;
      return { t: 'h', level, c: trimEdges(this.inlines(n, n.from, n.to, nested)) };
    }
    if (name === 'Task') return { t: 'p', c: trimEdges(this.inlines(n, n.from, n.to, true)) };
    if (name === 'Blockquote') return { t: 'quote', c: this.blocks(n, true) };
    if (name === 'HorizontalRule') return { t: 'hr' };
    if (name === 'FencedCode') {
      const text = n.getChild('CodeText');
      return { t: 'code', s: text ? this.src.slice(text.from, text.to) : '' };
    }
    if (name === 'BulletList' || name === 'OrderedList')
      return this.list(n, name === 'OrderedList');
    if (RAW_BLOCKS.has(name) || name.startsWith('Table')) return this.raw(n);
    return null;
  }

  private list(n: SyntaxNode, ordered: boolean): Block {
    const items: ListItem[] = [];
    for (let li = n.firstChild; li; li = li.nextSibling) {
      if (li.name !== 'ListItem') continue;
      const marker = li.getChild('Task')?.getChild('TaskMarker');
      items.push({
        task: marker
          ? {
              checked: /x/i.test(this.src.slice(marker.from, marker.to)),
              line: this.src.slice(0, marker.from).split('\n').length - 1,
            }
          : null,
        c: this.blocks(li, true),
      });
    }
    const mark = n.firstChild?.getChild('ListMark');
    const start = ordered && mark ? Number.parseInt(this.src.slice(mark.from, mark.to), 10) : 1;
    return { t: 'list', ordered, start: Number.isNaN(start) ? 1 : start, items };
  }

  private raw(n: SyntaxNode): Block {
    const lineStart = this.src.lastIndexOf('\n', n.from - 1) + 1;
    return { t: 'p', c: [{ t: 'text', s: this.src.slice(lineStart, n.to) }] };
  }

  /** Text between `from` and `to`; inside a quote or list the line prefixes are dropped. */
  private text(from: number, to: number, nested: boolean): string {
    const s = this.src.slice(from, to);
    return nested ? s.replace(/\n[ \t]*/g, '\n') : s;
  }

  private inlines(parent: SyntaxNode, from: number, to: number, nested: boolean): Inline[] {
    const out: Inline[] = [];
    const addText = (a: number, b: number) => {
      const s = this.text(a, b, nested);
      if (s) out.push({ t: 'text', s });
    };
    let pos = from;
    for (let c = parent.firstChild; c; c = c.nextSibling) {
      if (c.from < from || c.to > to) continue;
      if (c.from > pos) addText(pos, c.from);
      pos = c.to;
      const name = c.name;
      if (name === 'QuoteMark') {
        // The line's own prefix: drop it with the one space after it.
        trimLastText(out);
        if (this.src[pos] === ' ') pos += 1;
      } else if (HIDDEN.has(name)) {
        // Marks are hidden in the read view.
      } else if (INLINE_WRAPPERS[name]) {
        const t = INLINE_WRAPPERS[name];
        out.push({ t, c: this.inlines(c, c.from, c.to, nested) } as Inline);
      } else if (name === 'InlineCode') {
        const marks = c.getChildren('CodeMark');
        const open = marks[0];
        const close = marks.at(-1);
        out.push({
          t: 'code',
          s: open && close && open !== close ? this.src.slice(open.to, close.from) : '',
        });
      } else if (name === 'Link') {
        out.push(...this.link(c, nested));
      } else if (name === 'Autolink' || name === 'URL') {
        const url = name === 'URL' ? c : c.getChild('URL');
        const href = url ? this.src.slice(url.from, url.to) : '';
        out.push(...this.linkOrText(href, [{ t: 'text', s: href }], c));
      } else if (name === 'Escape') {
        out.push({ t: 'text', s: this.src.slice(c.from + 1, c.to) });
      } else if (name === 'HardBreak') {
        out.push({ t: 'text', s: '\n' });
      } else {
        out.push({ t: 'text', s: this.src.slice(c.from, c.to) });
      }
    }
    if (pos < to) addText(pos, to);
    return mergeText(out);
  }

  private link(n: SyntaxNode, nested: boolean): Inline[] {
    const marks = n.getChildren('LinkMark');
    const url = n.getChild('URL');
    const open = marks[0];
    const close = marks[1];
    if (!url || !open || !close || marks.length < 4) return [this.rawInline(n)];
    const href = this.src.slice(url.from, url.to);
    return this.linkOrText(href, this.inlines(n, open.to, close.from, nested), n);
  }

  private linkOrText(href: string, label: Inline[], n: SyntaxNode): Inline[] {
    return SAFE_HREF.test(href) ? [{ t: 'link', href, c: label }] : [this.rawInline(n)];
  }

  private rawInline(n: SyntaxNode): Inline {
    return { t: 'text', s: this.src.slice(n.from, n.to) };
  }
}

function trimLastText(inl: Inline[]) {
  const last = inl.at(-1);
  if (last?.t === 'text') last.s = last.s.replace(/[ \t]+$/, '');
}

function trimEdges(inl: Inline[]): Inline[] {
  const first = inl[0];
  if (first?.t === 'text') first.s = first.s.replace(/^[ \t]+/, '');
  const last = inl.at(-1);
  if (last?.t === 'text') last.s = last.s.replace(/[ \t]+$/, '');
  return inl.filter((i) => i.t !== 'text' || i.s !== '');
}

/** Joins neighbouring text nodes, so a dropped mark does not split a sentence. */
function mergeText(inl: Inline[]): Inline[] {
  const out: Inline[] = [];
  for (const i of inl) {
    const prev = out.at(-1);
    if (i.t === 'text' && prev?.t === 'text') prev.s += i.s;
    else out.push(i);
  }
  return out;
}
