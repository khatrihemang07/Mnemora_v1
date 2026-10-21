import { describe, expect, it } from 'vitest';
import { checklistProgress, notePreview, openSteps, plainTitle, tickAllSteps } from './notes';

describe('checklistProgress', () => {
  it('counts ticked and total items, nested ones too', () => {
    expect(checklistProgress('- [x] a\n- [ ] b\n  - [ ] c\n- [X] d')).toEqual({
      done: 2,
      total: 4,
    });
  });

  it('is none without items, and ignores look-alikes', () => {
    expect(checklistProgress('')).toBeNull();
    expect(
      checklistProgress('plain - [ ] text\n-[ ] no space\n```\n- [ ] in code\n```'),
    ).toBeNull();
  });

  it('counts open steps', () => {
    expect(openSteps('- [x] a\n- [ ] b\n- [ ] c')).toBe(2);
    expect(openSteps('- [x] a')).toBe(0);
    expect(openSteps('nothing')).toBe(0);
  });
});

describe('tickAllSteps', () => {
  it('ticks every open item and leaves the rest alone', () => {
    expect(tickAllSteps('Pack:\n- [x] tent\n- [ ] stove\n  * [ ] gas\ntext - [ ] x')).toBe(
      'Pack:\n- [x] tent\n- [x] stove\n  * [x] gas\ntext - [ ] x',
    );
    expect(tickAllSteps('nothing')).toBe('nothing');
  });
});

describe('notePreview', () => {
  it('is the first line of prose with the marks removed', () => {
    expect(notePreview('Route: the **ridge** loop from [the car park](https://x.y).\nMore')).toBe(
      'Route: the ridge loop from the car park.',
    );
    expect(notePreview('# A heading\nbody')).toBe('A heading');
    expect(notePreview('> quoted *text*')).toBe('quoted text');
    expect(notePreview('- plain item\n- [ ] step')).toBe('plain item');
    expect(notePreview('1. first\n2. second')).toBe('first');
    expect(notePreview('`code` and ==marks== ~~gone~~')).toBe('code and marks gone');
  });

  it('skips Checklist lines and blank lines', () => {
    expect(notePreview('\n\n- [ ] tent\n- [x] stove\n\nThen text')).toBe('Then text');
    expect(notePreview('- [ ] tent\n- [ ] stove')).toBe('');
    expect(notePreview('')).toBe('');
  });
});

describe('plainTitle', () => {
  it('drops the marks and keeps block syntax as text', () => {
    expect(plainTitle('Pay **rent** at [the bank](https://x.y)')).toBe('Pay rent at the bank');
    expect(plainTitle('# not a heading')).toBe('# not a heading');
  });
});
