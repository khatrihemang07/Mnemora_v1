/**
 * An Entry's text as paragraphs: blank lines separate them, single newlines stay inside one (the
 * view keeps them as line breaks). Markdown rendering comes later; this is plain text only.
 */
export function paragraphs(text: string): string[] {
  return text
    .split(/\n[ \t]*\n/)
    .map((p) => p.replace(/^\n+|\n+$/g, ''))
    .filter((p) => p.trim() !== '');
}
