import { describe, expect, it } from 'vitest';
import { parseMarkdown } from './ast';
import { toggleTaskLine } from './tasks';

describe('toggleTaskLine', () => {
  it('flips only the given line', () => {
    expect(toggleTaskLine('a\n- [ ] b\n- [ ] c', 2)).toBe('a\n- [ ] b\n- [x] c');
    expect(toggleTaskLine('- [x] b', 0)).toBe('- [ ] b');
    expect(toggleTaskLine('  + [X] b', 0)).toBe('  + [ ] b');
  });

  it('handles every list style the checklist view treats as a task', () => {
    for (const line of ['* [ ] a', '1. [ ] a', '2) [ ] a', '> - [ ] a', '- [ ]']) {
      expect(toggleTaskLine(line, 0), line).toBe(line.replace('[ ]', '[x]'));
    }
  });

  it('refuses lines that are not tasks', () => {
    for (const [body, line] of [
      ['plain', 0],
      ['- [ ]x', 0],
      ['- [] a', 0],
      ['[ ] a', 0],
      ['- [ ] a', 1],
    ] as const) {
      expect(toggleTaskLine(body, line), body).toBeNull();
    }
  });

  it('agrees with the parser on which line an item is on', () => {
    const body = 'Plan\n\n- [ ] one\n- [x] two\n  - [ ] nested';
    const [, list] = parseMarkdown(body);
    const lines = list?.t === 'list' ? list.items.map((i) => i.task?.line) : [];
    expect(lines).toEqual([2, 3]);
    for (const line of [2, 3, 4]) expect(toggleTaskLine(body, line)).not.toBeNull();
  });
});
