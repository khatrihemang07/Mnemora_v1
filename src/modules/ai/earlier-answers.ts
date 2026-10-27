import type { EntryView } from '../../lib/ipc/bindings';
import { withoutCitations } from './activity';
import { questionOfAnswers } from './answers';

/** How many characters of an answer the Branch picker shows before it cuts. */
export const ANSWER_START_CHARS = 120;
/** How many characters of a question the Branch picker shows before it cuts. */
const QUESTION_CHARS = 160;

/** One answer the person can branch from: the question it answers and the start of the answer. */
export interface EarlierAnswer {
  readonly answerId: string;
  readonly question: string;
  /** The start of the answer, one line; empty when the answer was stopped before its first word. */
  readonly start: string;
  readonly stopped: boolean;
}

/**
 * One line of text, with citation markers gone and whitespace collapsed, cut to `max` characters with
 * an ellipsis when it is longer.
 */
export function excerpt(text: string, max = ANSWER_START_CHARS): string {
  const plain = withoutCitations(text).split(/\s+/).filter(Boolean).join(' ');
  const chars = Array.from(plain);
  return chars.length > max ? `${chars.slice(0, max).join('').trimEnd()}…` : plain;
}

/**
 * The answers of a Branch that the person can branch from, oldest first. A Step that asks for Tools is
 * not an answer and is left out; a stopped answer is listed as it stands, marked stopped.
 */
export function earlierAnswers(entries: readonly EntryView[]): EarlierAnswer[] {
  const questions = questionOfAnswers(entries);
  const rows: EarlierAnswer[] = [];
  for (const entry of entries) {
    const question = questions.get(entry.id);
    if (entry.payload.kind !== 'assistant' || entry.payload.calls.length > 0 || !question) continue;
    rows.push({
      answerId: entry.id,
      question: excerpt(question.text, QUESTION_CHARS),
      start: excerpt(entry.payload.text),
      stopped: entry.payload.stop === 'aborted',
    });
  }
  return rows;
}
