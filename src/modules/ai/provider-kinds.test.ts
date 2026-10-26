import { describe, expect, it } from 'vitest';
import {
  addressFor,
  createRequest,
  freeOnlyNote,
  KINDS,
  keyChip,
  keyHint,
  kindOf,
  updateRequest,
} from './provider-kinds';

describe('the kinds a Provider can be added as', () => {
  it('offers the four kinds in order, each with a one-line description', () => {
    expect(KINDS.map((kind) => kind.id)).toEqual([
      'wrapper',
      'openai_compatible',
      'anthropic',
      'openrouter',
    ]);
    for (const kind of KINDS) expect(kind.description).not.toContain('\n');
  });

  it('fixes the address of Anthropic and OpenRouter and leaves the other two to the person', () => {
    expect(kindOf('anthropic')).toMatchObject({
      fixedAddress: true,
      address: 'https://api.anthropic.com/v1',
    });
    expect(kindOf('openrouter')).toMatchObject({
      fixedAddress: true,
      address: 'https://openrouter.ai/api/v1',
    });
    expect(kindOf('wrapper')).toMatchObject({
      fixedAddress: false,
      address: 'http://127.0.0.1:8080/v1',
    });
    expect(kindOf('openai_compatible').fixedAddress).toBe(false);
  });

  it('says which kinds take a key, and which of those need one', () => {
    expect(kindOf('wrapper').key).toBe('none');
    expect(kindOf('openai_compatible').key).toBe('optional');
    expect(kindOf('anthropic').key).toBe('required');
    expect(kindOf('openrouter').key).toBe('required');
  });

  it('only OpenRouter has paid Models', () => {
    expect(KINDS.filter((kind) => kind.paid).map((kind) => kind.id)).toEqual(['openrouter']);
  });

  it('sends a fixed address as it is, whatever was typed', () => {
    expect(addressFor(kindOf('anthropic'), 'https://elsewhere.example/v1')).toBe(
      'https://api.anthropic.com/v1',
    );
    expect(addressFor(kindOf('wrapper'), '  http://192.168.1.5:8080/v1  ')).toBe(
      'http://192.168.1.5:8080/v1',
    );
  });
});

describe('the requests the sheet sends', () => {
  it('creates with the kind chosen, never a fixed kind as the Wrapper', () => {
    expect(createRequest(kindOf('openrouter'), ' Mine ', '')).toEqual({
      kind: 'openrouter',
      name: 'Mine',
      address: 'https://openrouter.ai/api/v1',
    });
    expect(
      createRequest(kindOf('openai_compatible'), 'Local', 'http://127.0.0.1:11434/v1'),
    ).toEqual({
      kind: 'openai_compatible',
      name: 'Local',
      address: 'http://127.0.0.1:11434/v1',
    });
  });

  it('sends allow_paid on update for OpenRouter only, and false for every other kind', () => {
    expect(updateRequest(kindOf('openrouter'), 'R', '', true)).toEqual({
      name: 'R',
      address: 'https://openrouter.ai/api/v1',
      allow_paid: true,
    });
    expect(updateRequest(kindOf('openrouter'), 'R', '', false).allow_paid).toBe(false);
    expect(updateRequest(kindOf('wrapper'), 'W', 'http://127.0.0.1:8080/v1', true).allow_paid).toBe(
      false,
    );
    expect(updateRequest(kindOf('anthropic'), 'A', '', true).allow_paid).toBe(false);
  });
});

describe('the status shown for a Provider', () => {
  it('chips a missing key and a saved key, and shows nothing when no key is needed', () => {
    expect(keyChip('missing')).toEqual({ label: 'Needs a key', tone: 'warn' });
    expect(keyChip('saved')).toEqual({ label: 'Key saved', tone: 'ok' });
    expect(keyChip('not_needed')).toBeNull();
  });

  it('says Free Models only for OpenRouter while its paid switch is off', () => {
    expect(freeOnlyNote({ kind: 'openrouter', allow_paid: false })).toBe('Free Models only');
    expect(freeOnlyNote({ kind: 'openrouter', allow_paid: true })).toBeNull();
    expect(freeOnlyNote({ kind: 'anthropic', allow_paid: false })).toBeNull();
  });

  it('explains the key rule in the hint under the field', () => {
    expect(keyHint('required')).toContain('Required for this kind.');
    expect(keyHint('optional')).toContain('Optional for this kind.');
    expect(keyHint('required')).toContain('never shown again');
  });
});
