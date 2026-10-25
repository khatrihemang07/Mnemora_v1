/**
 * The AI Module in the browser mock: Conversations kept in memory and a scripted fake Provider.
 * It mirrors the Rust rules in `crates/ai` and `src-tauri/src/ai.rs`: a question must say
 * something, a Conversation answers one Run at a time, a missing one is not found, and a title is
 * the first question, collapsed and capped at 80 characters (`fixtures/ai-title.json` runs the same
 * cases in Rust and here).
 *
 * The scripted Provider answers every question with the same sentence, streamed word by word. A
 * question containing "fail" ends with an unreachable service instead, so the failure can be seen.
 */
import type {
  AiRunEvent,
  AppError,
  ConversationView,
  EntryView,
  RunEvent,
  RunRequest,
  RunStarted,
} from './bindings';

/** Longest title from a first question, in characters. */
export const AI_TITLE_CHARS = 80;

/** The title a Conversation gets from its first question. */
export function titleOf(question: string): string {
  const collapsed = question.split(/\s+/).filter(Boolean).join(' ');
  return Array.from(collapsed).slice(0, AI_TITLE_CHARS).join('').trimEnd();
}

/** Delay before each streamed word, so the answer visibly arrives. */
const WORD_DELAY_MS = 60;
/** Mock Provider name and Model, as an answer records them. */
const PROVIDER = { provider: 'Scripted', model: 'scripted-model' } as const;
const UNREACHABLE = 'Could not reach the AI service. Check its address and that it is running.';

interface StoredConversation {
  id: string;
  title: string;
  created_at: number;
  updated_at: number;
  entries: EntryView[];
}

export interface AiMockOptions {
  /** Where run events go; the page's bridge in the app, a recorder in tests. */
  emit: (event: AiRunEvent) => void;
  /** Schedules the next step of a Run; the browser's timer in the app. */
  later?: (fn: () => void, ms?: number) => void;
}

const aiError = (code: AppError['code'], message: string): AppError => ({
  code,
  message,
  details: null,
});

export function createAiMock({
  emit,
  later = (fn, ms = 0) => void setTimeout(fn, ms),
}: AiMockOptions) {
  const conversations = new Map<string, StoredConversation>();
  const answering = new Set<string>();
  let counter = 0;
  const nextId = (prefix: string) => `${prefix}-${++counter}`;

  function view(id: string): ConversationView {
    const found = conversations.get(id);
    if (!found) throw aiError('not_found', 'That Conversation no longer exists');
    // A copy, as real IPC gives: later saves must not change what the page already holds.
    return JSON.parse(JSON.stringify({ ...found, running: answering.has(id) })) as ConversationView;
  }

  function start(request: RunRequest): RunStarted {
    if (request.text.trim() === '') throw aiError('validation', 'Type a question first.');
    const continues = request.conversation_id !== null;
    const conversationId = request.conversation_id ?? nextId('conv');
    if (answering.has(conversationId)) {
      throw aiError('validation', 'This Conversation is still answering.');
    }
    const existing = conversations.get(conversationId);
    if (continues && !existing) throw aiError('not_found', 'That Conversation no longer exists');
    // As in Rust, the question is written here, before the Run's own steps run.
    const now = Date.now();
    const conversation = existing ?? startConversation(conversationId, request.text, now);
    const question = addEntry(conversation, { kind: 'user', text: request.text }, now);
    const runId = nextId('run');
    answering.add(conversationId);
    play(runId, conversation, question.id, request.text);
    return { run_id: runId, conversation_id: conversationId };
  }

  /** The Rust Run, step by step, each on its own turn of the event loop. */
  function play(runId: string, conversation: StoredConversation, questionId: string, text: string) {
    const conversationId = conversation.id;
    const say = (event: RunEvent) =>
      emit({ run_id: runId, conversation_id: conversationId, event });
    const end = (outcome: Extract<RunEvent, { type: 'runEnded' }>['outcome']) => {
      answering.delete(conversationId);
      say({ type: 'runEnded', outcome });
    };
    later(() => {
      say({ type: 'runStarted' });
      say({ type: 'entrySaved', entry_id: questionId, kind: 'user' });
      later(() => {
        say({ type: 'stepStarted' });
        if (text.includes('fail')) {
          return end({ type: 'failed', kind: 'unreachable', message: UNREACHABLE });
        }
        const words = answerFor(text).split(' ');
        words.forEach((word, index) => {
          later(
            () => {
              say({ type: 'textDelta', text: index === 0 ? word : ` ${word}` });
            },
            (index + 1) * WORD_DELAY_MS,
          );
        });
        later(
          () => {
            const at = Date.now();
            const answer = addEntry(
              conversation,
              { kind: 'assistant', text: words.join(' '), stop: 'stop', ...PROVIDER },
              at,
              questionId,
            );
            say({ type: 'entrySaved', entry_id: answer.id, kind: 'assistant' });
            end({ type: 'answered' });
          },
          (words.length + 1) * WORD_DELAY_MS,
        );
      });
    });
  }

  function startConversation(id: string, question: string, now: number): StoredConversation {
    const created: StoredConversation = {
      id,
      title: titleOf(question),
      created_at: now,
      updated_at: now,
      entries: [],
    };
    conversations.set(id, created);
    return created;
  }

  /** Appends an entry after the newest one (or after `parent`), as the Rust write does. */
  function addEntry(
    conversation: StoredConversation,
    payload: EntryView['payload'],
    now: number,
    parent?: string,
  ): EntryView {
    const last = conversation.entries.at(-1);
    const entry: EntryView = {
      id: nextId('entry'),
      parent_id: parent ?? last?.id ?? null,
      created_at: now,
      payload,
    };
    conversation.entries.push(entry);
    conversation.updated_at = now;
    return entry;
  }

  return {
    handles: (cmd: string) => cmd === 'ai_run_start' || cmd === 'ai_conversation_get',
    handle(cmd: string, args: Record<string, unknown>): RunStarted | ConversationView {
      if (cmd === 'ai_run_start') return start(args.request as RunRequest);
      return view(args.id as string);
    },
  };
}

/** The scripted Provider's answer to a question. */
function answerFor(question: string): string {
  return `This is a scripted answer to “${question.trim()}”. The mock reads nothing from your data.`;
}
