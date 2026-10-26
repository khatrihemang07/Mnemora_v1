import type { EntryView } from '../../lib/ipc/bindings';

/** The question an answer answers: its entry id (for Try again and Edit) and its text. */
export interface QuestionOfAnswer {
  readonly id: string;
  readonly text: string;
}

/**
 * The question each answer answers: the last question written before it. Try again, Edit and Save
 * this question all need the question an answer sits under.
 */
export function questionOfAnswers(entries: readonly EntryView[]): Map<string, QuestionOfAnswer> {
  const byAnswer = new Map<string, QuestionOfAnswer>();
  let question: QuestionOfAnswer | null = null;
  for (const entry of entries) {
    if (entry.payload.kind === 'user') question = { id: entry.id, text: entry.payload.text };
    else if (entry.payload.kind === 'assistant' && question !== null) {
      byAnswer.set(entry.id, question);
    }
  }
  return byAnswer;
}

/** The text of the question each answer answers (see `questionOfAnswers`). */
export function questionsByAnswer(entries: readonly EntryView[]): Map<string, string> {
  return new Map(
    [...questionOfAnswers(entries)].map(([answer, question]) => [answer, question.text]),
  );
}
