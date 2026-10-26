import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AiRunEvent, ConnectionTest, ModelList, ProviderView, RunRequest } from './bindings';
import addresses from './fixtures/ai-addresses.json';
import failures from './fixtures/ai-failures.json';
import historyRules from './fixtures/ai-history.json';
import markdownRules from './fixtures/ai-markdown.json';
import renameRules from './fixtures/ai-rename.json';
import rules from './fixtures/ai-title.json';
import {
  addressProblem,
  createAiMock,
  failureSentence,
  markdownOf,
  titleMatches,
  titleOf,
  validateTitle,
} from './mock-ai';

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

describe('shared fixtures for the Conversation rules', () => {
  it('renames to the trimmed title or refuses it, as the Rust rules do', () => {
    for (const rule of renameRules) {
      if ('error' in rule) {
        expect(caught(() => validateTitle(rule.title))).toMatchObject({
          code: 'validation',
          message: rule.error,
        });
      } else {
        expect(validateTitle(rule.title)).toBe(rule.saved);
      }
    }
  });

  it('matches the history search by title only, as the Rust rules do', () => {
    for (const rule of historyRules) {
      const matches = rule.titles.filter((title) => titleMatches(title, rule.query));
      expect(matches).toEqual(rule.matches);
    }
  });

  it('copies a Conversation as the Rust rules do', () => {
    for (const rule of markdownRules) {
      expect(markdownOf(rule.title, rule.entries as never)).toBe(rule.markdown);
    }
  });
});

describe('the mock Conversation and Saved question commands', () => {
  it('starts with demo Conversations that page at 30, and three Saved questions', () => {
    const mock = createAiMock({ emit: () => {} });
    const first = mock.handle('ai_conversation_list', { query: '', offset: 0, limit: 30 }) as {
      items: unknown[];
      more: boolean;
    };
    expect(first.items).toHaveLength(30);
    expect(first.more).toBe(true);
    const second = mock.handle('ai_conversation_list', { query: '', offset: 30, limit: 30 }) as {
      items: unknown[];
      more: boolean;
    };
    expect(second.items).toHaveLength(4);
    expect(second.more).toBe(false);
    expect(mock.handle('ai_saved_list', {})).toHaveLength(3);
  });

  it('deletes a Conversation out of the history and restores it whole', () => {
    const mock = createAiMock({ emit: () => {} });
    const id =
      (
        mock.handle('ai_conversation_list', { query: 'overdue', offset: 0, limit: 30 }) as {
          items: { id: string }[];
        }
      ).items[0]?.id ?? '';
    mock.handle('ai_conversation_delete', { id });
    expect(
      (
        mock.handle('ai_conversation_list', { query: 'overdue', offset: 0, limit: 30 }) as {
          items: unknown[];
        }
      ).items,
    ).toHaveLength(0);
    expect(caught(() => mock.handle('ai_conversation_get', { id }))).toMatchObject({
      code: 'not_found',
    });
    mock.handle('ai_conversation_restore', { id });
    expect(mock.handle('ai_conversation_get', { id })).toMatchObject({ title: 'What is overdue?' });
  });

  it('moves a Saved question between the neighbours it was given', () => {
    const mock = createAiMock({ emit: () => {} });
    const texts = () => (mock.handle('ai_saved_list', {}) as { text: string }[]).map((q) => q.text);
    const [first, second, third] = (mock.handle('ai_saved_list', {}) as { id: string }[]).map(
      (q) => q.id,
    );
    mock.handle('ai_saved_reorder', { id: third, afterId: first, beforeId: second });
    expect(texts()).toEqual([
      'Summarise my last 7 days',
      'What did I finish this month?',
      'What is overdue?',
    ]);
  });
});

describe('the mock Provider kinds, key states and paid Models', () => {
  let mock: ReturnType<typeof createAiMock>;

  beforeEach(() => {
    vi.useFakeTimers();
    mock = createAiMock({ emit: () => undefined });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  const add = (kind: string, name: string, address = '') =>
    mock.handle('ai_provider_create', {
      request: { kind, name, address },
    }) as ProviderView;
  const update = (id: string, name: string, address: string, allowPaid?: boolean) =>
    mock.handle('ai_provider_update', {
      id,
      request: { name, address, allow_paid: allowPaid },
    }) as ProviderView;
  const keySet = (id: string, key: string) =>
    mock.handle('ai_provider_key_set', { id, key }) as ProviderView;
  const keyClear = (id: string) => mock.handle('ai_provider_key_clear', { id }) as ProviderView;
  const test = (id: string) => mock.handle('ai_provider_test', { id }) as ConnectionTest;
  const models = (providerId: string, query = '') =>
    mock.handle('ai_models_search', { providerId, query }) as ModelList;
  const askWith = (providerId: string, model: string) =>
    mock.handle('ai_run_start', {
      request: { ...request('A question', null, providerId), model },
    });

  it('adds each of the four kinds and lists them as the kind they were made as', () => {
    add('wrapper', 'Wrapper', 'http://127.0.0.1:8080/v1');
    add('openai_compatible', 'Groq', 'https://api.groq.example/v1');
    add('anthropic', 'Claude');
    add('openrouter', 'Router');
    const listed = mock.handle('ai_provider_list', {}) as ProviderView[];
    expect(listed.map((p) => p.kind)).toEqual([
      'wrapper',
      'openai_compatible',
      'anthropic',
      'openrouter',
    ]);
    expect(listed[2]?.address).toBe('https://api.anthropic.com/v1');
    expect(listed[3]?.address).toBe('https://openrouter.ai/api/v1');
  });

  it('refuses another address for a fixed kind, and a kind it does not know', () => {
    expect(caught(() => add('anthropic', 'A', 'https://example.com/v1'))).toMatchObject({
      code: 'validation',
      message: 'This kind of Provider uses its own fixed address.',
    });
    expect(caught(() => add('nonsense', 'N', 'http://127.0.0.1:8080/v1'))).toMatchObject({
      message: 'Choose a kind of Provider first.',
    });
  });

  it('moves a required key from missing to saved and back, and never returns the key', () => {
    const anthropic = add('anthropic', 'Claude');
    expect(anthropic.key_state).toBe('missing');
    const saved = keySet(anthropic.id, '  sk-test-123  ');
    expect(saved.key_state).toBe('saved');
    expect(JSON.stringify(saved)).not.toContain('sk-test-123');
    expect(JSON.stringify(mock.handle('ai_provider_list', {}))).not.toContain('sk-test-123');
    expect(keyClear(anthropic.id).key_state).toBe('missing');
  });

  it('moves an optional key between not needed and saved', () => {
    const groq = add('openai_compatible', 'Groq', 'https://api.groq.example/v1');
    expect(groq.key_state).toBe('not_needed');
    expect(keySet(groq.id, 'gsk-1').key_state).toBe('saved');
    expect(keyClear(groq.id).key_state).toBe('not_needed');
  });

  it('refuses a key for the Wrapper, and an empty key', () => {
    const wrapper = add('wrapper', 'Wrapper', 'http://127.0.0.1:8080/v1');
    expect(caught(() => keySet(wrapper.id, 'k'))).toMatchObject({
      message: 'This kind of Provider does not use a key.',
    });
    const anthropic = add('anthropic', 'Claude');
    expect(caught(() => keySet(anthropic.id, '   '))).toMatchObject({
      message: 'Paste the key first.',
    });
  });

  it('fails a connection test and a send for a required key until the key is saved', () => {
    const anthropic = add('anthropic', 'Claude');
    expect(test(anthropic.id)).toMatchObject({ connected: false, failure: 'key_rejected' });
    expect(caught(() => askWith(anthropic.id, 'claude-haiku-5.5'))).toMatchObject({
      message: "Add this Provider's key first.",
    });
    keySet(anthropic.id, 'sk-ant-1');
    expect(test(anthropic.id)).toMatchObject({ connected: true });
    expect(askWith(anthropic.id, 'claude-haiku-5.5')).toMatchObject({
      conversation_id: expect.any(String),
    });
  });

  it('lists free and paid OpenRouter Models, and marks the paid ones', () => {
    const router = add('openrouter', 'Router');
    keySet(router.id, 'sk-or-1');
    const list = models(router.id);
    expect(list.models).toEqual([
      'deepseek-v3.2:free',
      'llama-3.3-70b:free',
      'claude-haiku-5.5',
      'gpt-5-mini',
    ]);
    expect(list.paid_models).toEqual(['claude-haiku-5.5', 'gpt-5-mini']);
    expect(models(router.id, 'gpt').paid_models).toEqual(['gpt-5-mini']);
  });

  it('refuses a paid Model until the paid switch is on, and a free one always runs', () => {
    const router = add('openrouter', 'Router');
    keySet(router.id, 'sk-or-1');
    expect(caught(() => askWith(router.id, 'gpt-5-mini'))).toMatchObject({
      code: 'validation',
      message: 'This is a paid Model. Turn on paid Models for this Provider to use it.',
    });
    expect(askWith(router.id, 'deepseek-v3.2:free')).toMatchObject({
      conversation_id: expect.any(String),
    });
    expect(update(router.id, 'Router', '', true).allow_paid).toBe(true);
    expect(askWith(router.id, 'gpt-5-mini')).toMatchObject({ conversation_id: expect.any(String) });
    expect(update(router.id, 'Router', '', false).allow_paid).toBe(false);
    expect(caught(() => askWith(router.id, 'gpt-5-mini'))).toMatchObject({
      message: 'This is a paid Model. Turn on paid Models for this Provider to use it.',
    });
  });

  it('keeps the paid switch off for kinds without paid Models', () => {
    const wrapper = add('wrapper', 'Wrapper', 'http://127.0.0.1:8080/v1');
    expect(update(wrapper.id, 'Wrapper', 'http://127.0.0.1:8080/v1', true).allow_paid).toBe(false);
  });

  it('removes the key of an OpenAI-compatible Provider when its address changes, and keeps it otherwise', () => {
    const groq = add('openai_compatible', 'Groq', 'https://api.groq.example/v1');
    keySet(groq.id, 'gsk-1');
    expect(update(groq.id, 'Groq renamed', 'https://api.groq.example/v1').key_state).toBe('saved');
    expect(update(groq.id, 'Groq', 'https://other.example/v1').key_state).toBe('not_needed');
  });
});
