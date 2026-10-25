import { describe, expect, it } from 'vitest';
import { enterBreaksLine } from './auto-textarea-enter';

describe('enterBreaksLine', () => {
  it('a single-line field swallows Enter and Shift+Enter', () => {
    expect(enterBreaksLine({ shiftKey: false, multiline: false, enterBreaks: false })).toBe(false);
    expect(enterBreaksLine({ shiftKey: true, multiline: false, enterBreaks: false })).toBe(false);
  });

  it('a multiline field adds a line on Shift+Enter only', () => {
    expect(enterBreaksLine({ shiftKey: true, multiline: true, enterBreaks: false })).toBe(true);
    expect(enterBreaksLine({ shiftKey: false, multiline: true, enterBreaks: false })).toBe(false);
  });

  it('enterBreaks makes plain Enter add a line, for the phone composer', () => {
    expect(enterBreaksLine({ shiftKey: false, multiline: true, enterBreaks: true })).toBe(true);
    expect(enterBreaksLine({ shiftKey: false, multiline: false, enterBreaks: true })).toBe(true);
  });
});
