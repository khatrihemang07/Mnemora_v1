/** Occurrence ids in the mock: the values Rust's `occurrence_id` (UUIDv5) gives. */
import { describe, expect, it } from 'vitest';
import { occurrenceId } from './mock-occurrence';

describe('occurrenceId', () => {
  it('derives the UUIDv5 of "{task}|{due}" in the pinned namespace', () => {
    expect(occurrenceId('task-1', '2026-11-01')).toBe('9d017efd-8e7a-5095-bbec-420895f73016');
    // Long enough to spill into a second SHA-1 block.
    expect(occurrenceId('0d3b1c1e-2f4a-4b6e-8c9d-1a2b3c4d5e6f', '2027-02-28')).toBe(
      '9bc1ca19-c352-5b1c-9ace-6569e4f49e1a',
    );
  });

  it('is the same every time and differs by day', () => {
    expect(occurrenceId('t', '2026-11-01')).toBe(occurrenceId('t', '2026-11-01'));
    expect(occurrenceId('t', '2026-11-01')).not.toBe(occurrenceId('t', '2026-11-02'));
  });
});
