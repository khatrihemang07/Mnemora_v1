import { describe, expect, it } from 'vitest';
import { openSteps, tickAllSteps } from '../../modules/todo/notes';
import fixture from './fixtures/todo-steps.json';

// The Todo widget's Rust rules (crates/todo/src/steps.rs) run the same cases.

describe('todo-steps fixture, against the app', () => {
  it.each(fixture.cases)('open steps: $name', ({ notes, open }) => {
    expect(openSteps(notes)).toBe(open);
  });

  it.each(fixture.cases)('tick all: $name', ({ notes, ticked }) => {
    expect(tickAllSteps(notes)).toBe(ticked);
  });
});
