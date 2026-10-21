import type { Parsed } from '../../lib/ipc/bindings';
import type { ParsedWhen, WhenContext } from '../../lib/ui/when-parse';
import { parseCapture } from './queries';

/**
 * What a "Type a date" entry means, from the parser's reading of it: a date, a time or both, and
 * nothing else. Text with anything more in it (a Repeat, a Priority, ordinary words), or two
 * dates to choose from, is not a date: the field says so rather than guessing.
 */
export function whenFromParse(text: string, parsed: Parsed | null): ParsedWhen | null {
  if (!parsed || parsed.spans.length === 0 || parsed.alternatives.length > 0) return null;
  if (parsed.spans.some((s) => s.kind !== 'date' && s.kind !== 'time')) return null;
  const covered = new Array<boolean>(text.length).fill(false);
  for (const s of parsed.spans) covered.fill(true, s.start, s.end);
  const left = [...text].some((c, i) => !covered[i] && !/[\s.,;:!?]/.test(c));
  if (left) return null;
  const hasDate = parsed.spans.some((s) => s.kind === 'date');
  const hasTime = parsed.spans.some((s) => s.kind === 'time');
  return {
    date: hasDate ? parsed.date : null,
    time: hasTime ? parsed.time : null,
  };
}

/** The When picker's reader: Rust's parser, asked about the typed text. */
export async function typedWhen(text: string, ctx: WhenContext): Promise<ParsedWhen | null> {
  const parsed = await parseCapture(text, { ignored: [], prefer: {} }, [], ctx);
  return whenFromParse(text, parsed);
}
