import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AiRunEvent, RunRequest } from './bindings';
import addresses from './fixtures/ai-addresses.json';
import failures from './fixtures/ai-failures.json';
import rules from './fixtures/ai-title.json';
import { addressProblem, createAiMock, failureSentence, titleOf } from './mock-ai';

/** What a rejected command threw (the mock throws its error object, as the IPC rejects). */
function caught(fn: () => unknown): unknown {
  try {
    fn();
  } catch (error) {
    return error;
  }
  throw new Error('the call did not throw');
}

function request(
  text: string,
  conversationId: string | null = null,
  providerId = 'provider-x',
): RunRequest {
  return {
    conversation_id: conversationId,
    text,
    provider_id: providerId,
    model: 'claude-haiku',
    local: {
      today: '2026-10-25',
      weekday: 'Sunday',
      time_zone: 'Europe/London',
      week_start: 'mon',
    },
  };
}

describe('titleOf (the same cases as the Rust tests)', () => {
  it.each(rules)('titles $question as $title', ({ question, title }) => {
    expect(titleOf(question)).toBe(title);
  });
});

describe('the AI browser mock', () => {
  let heard: AiRunEvent[];
  let mock: ReturnType<typeof createAiMock>;
  let providerId: string;

  beforeEach(() => {
    vi.useFakeTimers();
    heard = [];
    mock = createAiMock({ emit: (event) => heard.push(event) });
    providerId = (
      mock.handle('ai_provider_create', {
        request: { kind: 'wrapper', name: 'Scripted', address: 'http://127.0.0.1:9/v1' },
      }) as { id: string }
    ).id;
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function ask(text: string, conversationId: string | null = null) {
    return mock.handle('ai_run_start', { request: request(text, conversationId, providerId) }) as {
      run_id: string;
      conversation_id: string;
    };
  }

  it('answers in the order the Rust Run does and saves both entries', () => {
    const started = ask('What did I write about the move?');
    vi.runAllTimers();

    const labels = heard.map((e) =>
      e.event.type === 'entrySaved' ? `entrySaved:${e.event.kind}` : e.event.type,
    );
    expect(labels[0]).toBe('runStarted');
    expect(labels.slice(1, 4)).toEqual(['entrySaved:user', 'stepStarted', 'textDelta']);
    expect(labels.at(-2)).toBe('entrySaved:assistant');
    expect(labels.at(-1)).toBe('runEnded');
    expect(heard.every((e) => e.run_id === started.run_id)).toBe(true);

    const conversation = mock.handle('ai_conversation_get', { id: started.conversation_id }) as {
      title: string;
      running: boolean;
      entries: { payload: { kind: string; text: string } }[];
    };
    expect(conversation.title).toBe('What did I write about the move?');
    expect(conversation.running).toBe(false);
    // The Model is recorded first, as its own entry, then the question and the answer.
    expect(conversation.entries.map((e) => e.payload.kind)).toEqual([
      'model_change',
      'user',
      'assistant',
    ]);
    const streamed = heard
      .filter((e) => e.event.type === 'textDelta')
      .map((e) => (e.event.type === 'textDelta' ? e.event.text : ''))
      .join('');
    expect(conversation.entries[2]?.payload.text).toBe(streamed);
  });

  it('refuses a blank question and a question into a Conversation that is answering', () => {
    expect(caught(() => ask('   '))).toMatchObject({ code: 'validation' });
    const started = ask('First');
    expect(caught(() => ask('Second', started.conversation_id))).toMatchObject({
      message: 'This Conversation is still answering.',
    });
  });

  it('says a Conversation is not found when it is not there', () => {
    expect(caught(() => ask('Hello', 'missing'))).toMatchObject({ code: 'not_found' });
  });

  it('ends a question containing "fail" with a failure and keeps the question', () => {
    const started = ask('This will fail');
    vi.runAllTimers();

    const end = heard.at(-1);
    expect(end?.event.type).toBe('runEnded');
    if (end?.event.type !== 'runEnded') throw new Error('no end');
    expect(end.event.outcome.type).toBe('failed');
    const conversation = mock.handle('ai_conversation_get', { id: started.conversation_id }) as {
      entries: unknown[];
    };
    expect(conversation.entries).toHaveLength(2);
  });

  it.each([
    ['walk', ['01990000-0000-7000-8000-0000000000e1', '01990000-0000-7000-8000-0000000000e2']],
    ['overdue', ['01990000-0000-7000-8000-0000000000a1', '01990000-0000-7000-8000-0000000000a2']],
    ['week', ['01990000-0000-7000-8000-0000000000e1', '01990000-0000-7000-8000-0000000000e2']],
  ])('cites the items its Tools returned for a "%s" question', (word, ids) => {
    const started = ask(`What about my ${word}?`);
    vi.runAllTimers();

    const conversation = mock.handle('ai_conversation_get', { id: started.conversation_id }) as {
      entries: { payload: { kind: string; text?: string; sources?: { id: string }[] } }[];
    };
    const answer = conversation.entries.at(-1)?.payload;
    expect(answer?.kind).toBe('assistant');
    expect(answer?.sources?.map((source) => source.id)).toEqual(ids);
    expect(answer?.text).not.toContain('Nothing in your Journal or Tasks');
    expect(answer?.text).toContain(ids[0]);
  });
});

describe('the shared rules fixtures (the same cases as the Rust tests)', () => {
  it.each(addresses)('checks the address $address', ({ address, ok, reason }) => {
    const expected = ok
      ? null
      : reason === 'plain_http'
        ? 'plain_http'
        : reason === 'scheme'
          ? 'scheme'
          : 'invalid';
    expect(addressProblem(address)).toBe(expected);
  });

  it.each(failures)('words the $kind failure as the fixture does', ({ kind, message }) => {
    expect(failureSentence(kind as never)).toBe(message);
  });
});
