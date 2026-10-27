import { describe, expect, it } from 'vitest';
import type { ModelUsage, ProviderView } from '../../lib/ipc/bindings';
import { type ChoiceSources, effectiveChoice } from './choice';

const provider = (id: string): ProviderView => ({
  id,
  name: id,
  kind: 'wrapper',
  address: 'http://127.0.0.1:8080/v1',
  key_state: 'not_needed',
  created_at: 0,
  updated_at: 0,
});

const usage = (providerId: string, model: string): ModelUsage => ({
  recent: [{ provider_id: providerId, provider_name: providerId, model, uses: 1, last_used_at: 1 }],
  most_used: [],
});

const base: ChoiceSources = {
  picked: null,
  conversation: null,
  usage: null,
  providers: [provider('mac'), provider('server')],
};

describe('the Model a question starts with', () => {
  it('prefers the Model picked on the page', () => {
    const choice = effectiveChoice({
      ...base,
      picked: { providerId: 'server', model: 'codex-luna' },
      conversation: { providerId: 'mac', model: 'claude-haiku' },
      usage: usage('mac', 'claude-sonnet'),
    });
    expect(choice).toEqual({ providerId: 'server', model: 'codex-luna' });
  });

  it('then uses the open Conversation’s Model', () => {
    const choice = effectiveChoice({
      ...base,
      conversation: { providerId: 'mac', model: 'claude-haiku' },
      usage: usage('server', 'codex-luna'),
    });
    expect(choice).toEqual({ providerId: 'mac', model: 'claude-haiku' });
  });

  it('then the most recently used Model on this device', () => {
    expect(effectiveChoice({ ...base, usage: usage('server', 'codex-luna') })).toEqual({
      providerId: 'server',
      model: 'codex-luna',
    });
  });

  it('is none when nothing usable is known, so the person chooses', () => {
    expect(effectiveChoice(base)).toBeNull();
  });

  it('skips a choice that names a Provider this device no longer has', () => {
    const choice = effectiveChoice({
      ...base,
      picked: { providerId: 'gone', model: 'x' },
      usage: usage('mac', 'claude-haiku'),
    });
    expect(choice).toEqual({ providerId: 'mac', model: 'claude-haiku' });
  });
});
