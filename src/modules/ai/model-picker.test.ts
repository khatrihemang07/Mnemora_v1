import { describe, expect, it } from 'vitest';
import {
  isFreeModel,
  matchingModels,
  modelRowState,
  PAID_MODEL_REASON,
  typedModelId,
  usesOf,
} from './model-picker';

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

describe('paid Models in the picker', () => {
  const paidIds = new Set(['gpt-5-mini']);

  it('reads a free route by its :free suffix', () => {
    expect(isFreeModel('deepseek-v3.2:free')).toBe(true);
    expect(isFreeModel('gpt-5-mini')).toBe(false);
  });

  it('disables a paid row, with its reason, while the switch is off', () => {
    expect(modelRowState('gpt-5-mini', { paidKind: true, paidIds, allowPaid: false })).toEqual({
      paid: true,
      disabled: true,
      reason: PAID_MODEL_REASON,
    });
    expect(PAID_MODEL_REASON).toBe(
      'Paid Model. Turn on paid Models for this Provider to choose it.',
    );
  });

  it('keeps a paid row selectable once the switch is on', () => {
    expect(modelRowState('gpt-5-mini', { paidKind: true, paidIds, allowPaid: true })).toEqual({
      paid: true,
      disabled: false,
      reason: null,
    });
  });

  it('never disables a free row', () => {
    expect(
      modelRowState('deepseek-v3.2:free', { paidKind: true, paidIds, allowPaid: false }),
    ).toMatchObject({ paid: false, disabled: false, reason: null });
  });

  it('treats an unlisted paid-looking id as paid for OpenRouter, and no id as paid for a kind without paid Models', () => {
    expect(
      modelRowState('claude-haiku-5.5', { paidKind: true, paidIds: new Set(), allowPaid: false })
        .disabled,
    ).toBe(true);
    expect(
      modelRowState('gpt-5-mini', { paidKind: false, paidIds, allowPaid: false }).disabled,
    ).toBe(false);
  });
});
