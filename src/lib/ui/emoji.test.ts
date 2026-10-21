import { describe, expect, it } from 'vitest';
import { CURATED_EMOJI, firstEmoji } from './emoji';

describe('firstEmoji', () => {
  it('finds a plain emoji among other text', () => {
    expect(firstEmoji('Home 🏠 sweet')).toBe('🏠');
    expect(firstEmoji('🏠🛒')).toBe('🏠');
  });

  it('keeps a whole sequence: variation selector, skin tone, joiners, flags', () => {
    expect(firstEmoji('x✈️')).toBe('✈️');
    expect(firstEmoji('👍🏽')).toBe('👍🏽');
    expect(firstEmoji('👨‍👩‍👧')).toBe('👨‍👩‍👧');
    expect(firstEmoji('🇮🇳')).toBe('🇮🇳');
  });

  it('is null when there is none', () => {
    expect(firstEmoji('')).toBeNull();
    expect(firstEmoji('abc 123')).toBeNull();
  });
});

describe('the curated grid', () => {
  it('holds 48 distinct emoji, each a single sequence', () => {
    expect(CURATED_EMOJI).toHaveLength(48);
    expect(new Set(CURATED_EMOJI).size).toBe(48);
    for (const e of CURATED_EMOJI) expect(firstEmoji(e)).toBe(e);
  });
});
