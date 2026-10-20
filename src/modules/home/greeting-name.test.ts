import { describe, expect, it } from 'vitest';
import { homeName } from './greeting-name';

describe('homeName', () => {
  it('prefers the display name', () => expect(homeName('Sam', 'Alex')).toBe('Sam'));
  it('falls back to the OS first name', () => expect(homeName('  ', 'Alex')).toBe('Alex'));
  it('falls back to no name', () => expect(homeName('', undefined)).toBe(''));
});
