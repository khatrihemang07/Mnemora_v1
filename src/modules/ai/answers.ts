import type { EntryView } from '../../lib/ipc/bindings';

/**
 * The question each answer answers: the last question written before it. A Save-this-question
 * action needs the text of the question an answer sits under.
 */
export function questionsByAnswer(entries: readonly EntryView[]): Map<string, string> {
  const byAnswer = new Map<string, string>();
  let question: string | null = null;
  for (const entry of entries) {
    if (entry.payload.kind === 'user') question = entry.payload.text;
    else if (entry.payload.kind === 'assistant' && question !== null) {
      byAnswer.set(entry.id, question);
    }
  }
  return byAnswer;
}
