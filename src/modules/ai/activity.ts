import type { EntryView, Source } from '../../lib/ipc/bindings';

/** One Tool step as the activity line shows it. */
export interface ActivityStep {
  readonly name: string;
  readonly summary: string | null;
  readonly isError: boolean;
}

/** The live line while a Run works: what it is doing, and how many steps are done. */
export function runningLabel(done: number): string {
  if (done === 0) return 'Reading your journal and tasks…';
  return `Reading your journal and tasks… · ${done} ${plural('step', done)} done`;
}

/** The collapsed line once a Run has finished its steps. */
export function completedLabel(count: number): string {
  return `Completed ${count} ${plural('step', count)}`;
}

function plural(word: string, count: number): string {
  return count === 1 ? word : `${word}s`;
}

/** Where a cited item lives in the app: a Journal Page by its date, or a Task by its id. */
export function openHref(source: Source): string {
  if (source.module === 'todo') return `/todo?task=${encodeURIComponent(source.id)}`;
  return source.date ? `/journal/${source.date}` : '/journal';
}

/** The Module a cited item belongs to, as the person reads it. */
export function sourceModule(source: Source): string {
  return source.module === 'todo' ? 'Todo' : 'Journal';
}

const CITATION = /\s*\[[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\]/g;

/** The answer's text without the id markers the Model wrote; the Sources chips stand for them. */
export function withoutCitations(text: string): string {
  return text.replace(CITATION, '');
}

/**
 * The Tool steps that belong to each question: the tool results saved after it, up to the next
 * question. Keyed by the question's entry id.
 */
export function stepsByQuestion(entries: readonly EntryView[]): Map<string, ActivityStep[]> {
  const byQuestion = new Map<string, ActivityStep[]>();
  let question: string | null = null;
  for (const entry of entries) {
    if (entry.payload.kind === 'user') {
      question = entry.id;
      byQuestion.set(entry.id, []);
    } else if (entry.payload.kind === 'tool_result' && question !== null) {
      byQuestion.get(question)?.push({
        name: entry.payload.name,
        summary: entry.payload.summary,
        isError: entry.payload.is_error,
      });
    }
  }
  return byQuestion;
}
