import { type Block, type Inline, parseMarkdown } from '../../lib/ui/markdown/ast';
import { parseInlineMarkdown } from '../../lib/ui/markdown/inline';
import { toggleTaskLine } from '../../lib/ui/markdown/tasks';

/** Every Checklist item in some blocks, nested ones included, as `{ line, checked }`. */
function checklistItems(blocks: Block[]): { line: number; checked: boolean }[] {
  return blocks.flatMap((b): { line: number; checked: boolean }[] => {
    if (b.t === 'quote') return checklistItems(b.c);
    if (b.t !== 'list') return [];
    return b.items.flatMap((item) => [
      ...(item.task ? [item.task] : []),
      ...checklistItems(item.c),
    ]);
  });
}

/** The Checklist of some notes as ticked and total items, or none when there are no items. */
export function checklistProgress(notes: string): { done: number; total: number } | null {
  const items = checklistItems(parseMarkdown(notes));
  if (items.length === 0) return null;
  return { done: items.filter((i) => i.checked).length, total: items.length };
}

/** How many Checklist items are still unticked. */
export const openSteps = (notes: string): number => {
  const progress = checklistProgress(notes);
  return progress ? progress.total - progress.done : 0;
};

/** The notes with every Checklist item ticked: "Tick all and complete". */
export function tickAllSteps(notes: string): string {
  let out = notes;
  for (const item of checklistItems(parseMarkdown(notes))) {
    if (!item.checked) out = toggleTaskLine(out, item.line) ?? out;
  }
  return out;
}

/** The text of inline nodes with every mark removed. */
function plain(nodes: Inline[]): string {
  return nodes.map((n) => (n.t === 'text' || n.t === 'code' ? n.s : plain(n.c))).join('');
}

/** The first line of the plain text in some blocks, skipping Checklist items. */
function firstProse(blocks: Block[]): string {
  for (const b of blocks) {
    let text = '';
    if (b.t === 'p' || b.t === 'h') text = plain(b.c);
    else if (b.t === 'quote') text = firstProse(b.c);
    else if (b.t === 'list') {
      for (const item of b.items) {
        if (!item.task) text = firstProse(item.c);
        if (text) break;
      }
    }
    const line = text.split('\n').find((l) => l.trim() !== '');
    if (line) return line.trim();
  }
  return '';
}

/**
 * What a row shows of the notes: the first line of prose with the Markdown marks removed.
 * Checklist items are skipped (the "1/3" covers them); notes that are only a Checklist show
 * nothing.
 */
export const notePreview = (notes: string): string => firstProse(parseMarkdown(notes));

/** A title without its Markdown marks, for a sentence that quotes it. */
export const plainTitle = (title: string): string => plain(parseInlineMarkdown(title));
