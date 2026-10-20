/**
 * Rich-text paste: turns clipboard HTML into the Markdown the Journal supports (bold, italic,
 * strikethrough, highlight, inline code, links, headings, lists, quotes, code blocks, dividers).
 * A small tolerant tokenizer instead of a DOM or a dependency, so it runs (and tests) anywhere.
 * The HTML is only ever read as text here; nothing from it is rendered.
 */

interface El {
  tag: string;
  attrs: Record<string, string>;
  children: Node[];
}
type Node = string | El;

const VOID = new Set(['br', 'hr', 'img', 'input', 'meta', 'link', 'col', 'wbr', 'source']);
const SKIP = new Set(['script', 'style', 'head', 'title', 'template', 'svg', 'noscript']);
/** Tags that mean the clipboard holds real structure worth converting. */
const SEMANTIC =
  /<(a|strong|b|em|i|del|s|strike|mark|code|pre|h[1-6]|ul|ol|li|blockquote|hr)[\s>/]/i;
const ENTITIES: Record<string, string> = {
  amp: '&',
  lt: '<',
  gt: '>',
  quot: '"',
  apos: "'",
  nbsp: ' ',
  ndash: '–',
  mdash: '—',
  hellip: '…',
};
const SAFE_HREF = /^(https?:|mailto:|tel:)/i;
const TAG = /<!--[\s\S]*?-->|<(\/?)([a-zA-Z][a-zA-Z0-9-]*)((?:"[^"]*"|'[^']*'|[^>"'])*)>|[^<]+|</g;
const ATTR = /([a-zA-Z_:][-a-zA-Z0-9_:.]*)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+)))?/g;

/** Whether the HTML is worth converting; plain wrappers (spans, divs) paste as plain text. */
export function isRichHtml(html: string): boolean {
  return SEMANTIC.test(html);
}

function decode(text: string): string {
  return text.replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);/gi, (whole, name: string) => {
    if (name.startsWith('#')) {
      const code =
        name[1]?.toLowerCase() === 'x' ? Number.parseInt(name.slice(2), 16) : Number(name.slice(1));
      return Number.isFinite(code) && code > 0 && code <= 0x10ffff
        ? String.fromCodePoint(code)
        : whole;
    }
    return ENTITIES[name.toLowerCase()] ?? whole;
  });
}

function parseAttrs(source: string): Record<string, string> {
  const attrs: Record<string, string> = {};
  for (const m of source.matchAll(ATTR)) {
    const name = m[1]?.toLowerCase();
    if (name) attrs[name] = decode(m[2] ?? m[3] ?? m[4] ?? '');
  }
  return attrs;
}

function parse(html: string): El {
  const root: El = { tag: '#root', attrs: {}, children: [] };
  const stack: El[] = [root];
  const top = () => stack[stack.length - 1] ?? root;
  for (const m of html.matchAll(TAG)) {
    const token = m[0];
    if (token.startsWith('<!--')) continue;
    const name = m[2]?.toLowerCase();
    if (name === undefined) {
      top().children.push(token);
    } else if (m[1]) {
      const at = stack.map((e) => e.tag).lastIndexOf(name);
      if (at > 0) stack.length = at;
    } else {
      const el: El = { tag: name, attrs: parseAttrs(m[3] ?? ''), children: [] };
      top().children.push(el);
      if (!VOID.has(name) && !token.endsWith('/>')) stack.push(el);
    }
  }
  return root;
}

const isEl = (n: Node): n is El => typeof n !== 'string';

function style(el: El, prop: string): string {
  const m = new RegExp(`(?:^|;)\\s*${prop}\\s*:\\s*([^;]+)`, 'i').exec(el.attrs.style ?? '');
  return m?.[1]?.trim().toLowerCase() ?? '';
}

/** Whether a tag or its inline style makes the text bold, italic or struck. */
function emphasis(el: El): { bold: boolean; italic: boolean; strike: boolean } {
  const weight = style(el, 'font-weight');
  const numeric = Number.parseInt(weight, 10);
  const styledBold = weight === 'bold' || weight === 'bolder' || numeric >= 600;
  // Google Docs wraps everything in <b style="font-weight:normal">.
  const normal = weight === 'normal' || (numeric > 0 && numeric < 600);
  return {
    bold: (el.tag === 'strong' || el.tag === 'b') && !normal ? true : styledBold,
    italic: el.tag === 'em' || el.tag === 'i' || style(el, 'font-style') === 'italic',
    strike:
      el.tag === 'del' ||
      el.tag === 's' ||
      el.tag === 'strike' ||
      style(el, 'text-decoration').includes('line-through'),
  };
}

/** Wraps `inner` in `mark`, keeping surrounding spaces outside so the Markdown stays valid. */
function wrap(inner: string, mark: string): string {
  const core = inner.trim();
  if (core === '') return inner;
  const lead = inner.slice(0, inner.length - inner.trimStart().length);
  const trail = inner.slice(inner.trimEnd().length);
  return `${lead}${mark}${core}${mark}${trail}`;
}

function rawText(nodes: Node[]): string {
  return nodes
    .map((n) =>
      typeof n === 'string'
        ? decode(n)
        : n.tag === 'br'
          ? '\n'
          : SKIP.has(n.tag)
            ? ''
            : rawText(n.children),
    )
    .join('');
}

function inline(nodes: Node[]): string {
  return nodes.map(inlineNode).join('');
}

function inlineNode(n: Node): string {
  if (typeof n === 'string') return decode(n).replace(/\s+/g, ' ');
  if (SKIP.has(n.tag)) return '';
  if (n.tag === 'br') return '\n';
  if (n.tag === 'input')
    return n.attrs.type === 'checkbox' ? ('checked' in n.attrs ? '[x] ' : '[ ] ') : '';
  if (n.tag === 'img') return '';
  if (n.tag === 'code') return wrapCode(rawText(n.children).replace(/\s+/g, ' '));
  if (n.tag === 'a') return link(n);

  let out = inline(n.children);
  const e = emphasis(n);
  if (n.tag === 'mark') out = wrap(out, '==');
  if (e.strike) out = wrap(out, '~~');
  if (e.italic) out = wrap(out, '*');
  if (e.bold) out = wrap(out, '**');
  return n.tag === 'td' || n.tag === 'th' ? `${out} ` : out;
}

function wrapCode(text: string): string {
  const core = text.trim();
  if (core === '') return '';
  return core.includes('`') ? `\`\` ${core} \`\`` : wrap(text, '`');
}

function link(n: El): string {
  const text = inline(n.children);
  const href = n.attrs.href?.trim() ?? '';
  const label = text.trim();
  if (!SAFE_HREF.test(href) || label === '') return text;
  if (label === href) return text;
  const lead = text.slice(0, text.length - text.trimStart().length);
  const trail = text.slice(text.trimEnd().length);
  return `${lead}[${label}](${href.replace(/\)/g, '%29')})${trail}`;
}

interface Block {
  text: string;
  /** Paragraph-like blocks are separated by a blank line; line-like ones (div) by a newline. */
  loose: boolean;
}

const CONTAINERS = new Set([
  'div',
  'section',
  'article',
  'main',
  'header',
  'footer',
  'nav',
  'aside',
  'figure',
  'table',
  'thead',
  'tbody',
  'tfoot',
  'tr',
  'body',
  'html',
  'center',
  'details',
  'summary',
]);

function blocks(nodes: Node[]): Block[] {
  const out: Block[] = [];
  let run: Node[] = [];
  const flush = () => {
    const text = inline(run)
      .replace(/[ \t]*\n[ \t]*/g, '\n')
      .trim();
    if (text !== '') out.push({ text, loose: false });
    run = [];
  };
  for (const n of nodes) {
    if (!isEl(n) || !isBlockTag(n.tag)) {
      run.push(n);
      continue;
    }
    flush();
    out.push(...blockOf(n));
  }
  flush();
  return out;
}

function isBlockTag(tag: string): boolean {
  return CONTAINERS.has(tag) || /^(p|h[1-6]|ul|ol|li|blockquote|pre|hr|dl|dt|dd)$/.test(tag);
}

function blockOf(el: El): Block[] {
  const tag = el.tag;
  if (SKIP.has(tag)) return [];
  if (tag === 'hr') return [{ text: '---', loose: true }];
  if (tag === 'pre') {
    const code = rawText(el.children).replace(/^\n+|\n+$/g, '');
    return [{ text: `\`\`\`\n${code}\n\`\`\``, loose: true }];
  }
  const heading = /^h([1-6])$/.exec(tag);
  if (heading) {
    const text = inline(el.children)
      .replace(/\s*\n\s*/g, ' ')
      .trim();
    return text === ''
      ? []
      : [{ text: `${'#'.repeat(Math.min(3, Number(heading[1])))} ${text}`, loose: true }];
  }
  if (tag === 'ul' || tag === 'ol') {
    const text = list(el);
    return text === '' ? [] : [{ text, loose: true }];
  }
  if (tag === 'blockquote') {
    const inner = join(blocks(el.children));
    if (inner === '') return [];
    const text = inner
      .split('\n')
      .map((l) => (l === '' ? '>' : `> ${l}`))
      .join('\n');
    return [{ text, loose: true }];
  }
  const inner = blocks(el.children);
  if (tag === 'p' || tag === 'li' || tag === 'dd' || tag === 'dt') {
    return inner.map((b) => ({ ...b, loose: true }));
  }
  return inner;
}

function list(el: El): string {
  const ordered = el.tag === 'ol';
  let n = Number.parseInt(el.attrs.start ?? '1', 10);
  if (!Number.isFinite(n)) n = 1;
  const lines: string[] = [];
  for (const li of el.children) {
    if (!isEl(li) || li.tag !== 'li') continue;
    const marker = ordered ? `${n}. ` : '- ';
    n += 1;
    const own: Node[] = [];
    const nested: string[] = [];
    for (const c of li.children) {
      if (isEl(c) && (c.tag === 'ul' || c.tag === 'ol')) nested.push(list(c));
      else own.push(c);
    }
    const text = inline(own)
      .replace(/\s*\n\s*/g, ' ')
      .replace(/\s+/g, ' ')
      .trim();
    lines.push(`${marker}${text}`.trimEnd());
    const pad = ' '.repeat(marker.length);
    for (const sub of nested) for (const l of sub.split('\n')) lines.push(pad + l);
  }
  return lines.join('\n');
}

function join(list: Block[]): string {
  let out = '';
  list.forEach((b, i) => {
    const prev = list[i - 1];
    if (prev) out += prev.loose || b.loose ? '\n\n' : '\n';
    out += b.text;
  });
  return out;
}

/** Converts clipboard HTML to Markdown. Unsupported markup falls back to its plain text. */
export function htmlToMarkdown(html: string): string {
  const text = join(blocks(parse(html).children));
  return text
    .replace(/[ \t]+$/gm, '')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}
