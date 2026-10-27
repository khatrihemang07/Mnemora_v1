import { describe, expect, it } from 'vitest';
import type { RunOutcome } from '../../lib/ipc/bindings';
import { announcementFor } from './announce';

describe('the Run-end announcement', () => {
  it('says "Response complete" once an answer is saved', () => {
    expect(announcementFor({ type: 'answered' })).toBe('Response complete');
  });

  it('says the answer was stopped, not complete', () => {
    expect(announcementFor({ type: 'stopped' })).toBe('Response stopped');
  });

  it('gives the one plain sentence of a failure and never a raw Provider error', () => {
    const outcome: RunOutcome = {
      type: 'failed',
      kind: 'unreachable',
      message: 'The Provider cannot be reached.',
    };
    expect(announcementFor(outcome)).toBe('Response failed: The Provider cannot be reached.');
  });
});
