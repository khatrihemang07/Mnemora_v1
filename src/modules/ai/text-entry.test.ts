import { describe, expect, it } from 'vitest';
import { PLAIN_ENTRY } from './text-entry';

describe('plain entry for Model ids, addresses and keys', () => {
  it('turns off capitals, autocorrect and spellcheck, which mangle ids and keys', () => {
    expect(PLAIN_ENTRY).toEqual({
      autocapitalize: 'off',
      autocorrect: 'off',
      autocomplete: 'off',
      spellcheck: false,
    });
  });
});
