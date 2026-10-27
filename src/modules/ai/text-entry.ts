/**
 * Attributes for the text a person types that the keyboard must not change: Model ids, addresses
 * and keys. Spread onto the Input so the three fields cannot drift apart.
 */
export const PLAIN_ENTRY = {
  autocapitalize: 'off',
  autocorrect: 'off',
  autocomplete: 'off',
  spellcheck: false,
} as const;
