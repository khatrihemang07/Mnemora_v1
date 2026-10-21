import { type Block, type Inline, parseMarkdown } from './ast';

/**
 * Where a line would start a block: a heading, quote, bullet, numbered item, fence, HTML, rule or
 * link reference. One backslash in front of the marker turns it into the plain text it looks like.
 */
const BLOCK_STARTS: readonly RegExp[] = [
  /^(\s*)(#{1,6}(?=\s|$))/,
  /^(\s*)(>)/,
  /^(\s*)([-*+](?=\s|$))/,
  /^(\s*\d{1,9})([.)](?=\s|$))/,
  /^(\s*)(```|~~~)/,
  /^(\s*)(<)/,
  /^(\s*)([-*_](?=(?:\s*[-*_]){2,}\s*$))/,
  /^(\s*)(\[[^\]]*\]:\s)/,
];

/** One line with a backslash before whatever would make it a block, so only inline marks remain. */
function escapeLine(src: string): string {
  for (const start of BLOCK_STARTS) {
    const m = start.exec(src);
    if (m) return `${m[1]}\\${src.slice((m[1] ?? '').length)}`;
  }
  return src;
}

/** Every line of the text, so a comment with line breaks stays inline text too. */
const withoutBlockSyntax = (src: string): string => src.split('\n').map(escapeLine).join('\n');

/** Strikethrough and highlight are off for now: their marks come back as plain text. */
function plainDeletions(nodes: Inline[]): Inline[] {
  return nodes.flatMap((n): Inline[] => {
    if (n.t === 'del')
      return [{ t: 'text', s: '~~' }, ...plainDeletions(n.c), { t: 'text', s: '~~' }];
    if (n.t === 'mark')
      return [{ t: 'text', s: '==' }, ...plainDeletions(n.c), { t: 'text', s: '==' }];
    if (n.t === 'strong' || n.t === 'em' || n.t === 'link')
      return [{ ...n, c: plainDeletions(n.c) }];
    return [n];
  });
}

/** Joins neighbouring text, at every depth, so a restored mark does not split a word. */
function mergeText(nodes: Inline[]): Inline[] {
  const out: Inline[] = [];
  for (const n of nodes) {
    const last = out.at(-1);
    if (n.t === 'text' && last?.t === 'text') last.s += n.s;
    else if (n.t === 'strong' || n.t === 'em' || n.t === 'link')
      out.push({ ...n, c: mergeText(n.c) });
    else out.push(n.t === 'text' ? { ...n } : n);
  }
  return out;
}

/**
 * The inline reading of one line of text (a Task title, a comment) with the Journal's parser: bold,
 * italic, code, links and bare URLs are marked up; block syntax (`#`, `-`, `>`, `1.`) stays as the
 * text it is. Strikethrough and highlight are not rendered yet.
 */
export function parseInlineMarkdown(src: string): Inline[] {
  if (src === '') return [];
  const blocks: Block[] = parseMarkdown(withoutBlockSyntax(src));
  // Blank lines split paragraphs; they are joined back with the line breaks they were.
  if (blocks.length === 0 || blocks.some((b) => b.t !== 'p')) return [{ t: 'text', s: src }];
  const inlines = blocks.flatMap((b, i): Inline[] =>
    b.t === 'p' ? [...(i > 0 ? [{ t: 'text' as const, s: '\n\n' }] : []), ...b.c] : [],
  );
  return mergeText(plainDeletions(inlines));
}
