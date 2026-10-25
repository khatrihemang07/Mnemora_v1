import { describe, expect, it } from 'vitest';
import { matchingModels, typedModelId, usesOf } from './model-picker';

describe('the Model picker’s lists', () => {
  it('accepts any typed id, trimmed, and nothing when the box is empty', () => {
    expect(typedModelId('  openai/gpt-5-mini  ')).toBe('openai/gpt-5-mini');
    expect(typedModelId('   ')).toBeNull();
  });

  it('filters by any part of the id, case-insensitively, and keeps the Provider’s order', () => {
    const models = ['claude-haiku', 'claude-sonnet', 'codex-luna'];
    expect(matchingModels(models, 'CLAUDE')).toEqual(['claude-haiku', 'claude-sonnet']);
    expect(matchingModels(models, '')).toEqual(models);
    expect(matchingModels(models, 'luna')).toEqual(['codex-luna']);
  });

  it('keeps only the uses of one Provider', () => {
    const uses = [
      { provider_id: 'a', provider_name: 'A', model: 'x', uses: 1, last_used_at: 1 },
      { provider_id: 'b', provider_name: 'B', model: 'y', uses: 2, last_used_at: 2 },
    ];
    expect(usesOf(uses, 'b').map((u) => u.model)).toEqual(['y']);
  });
});
