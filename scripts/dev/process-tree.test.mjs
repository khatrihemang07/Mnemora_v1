import { describe, expect, it } from 'vitest';
import { descendantPids } from './process-tree.mjs';

describe('descendantPids', () => {
  it('finds every process under the root, deepest first, and not the root itself', () => {
    const ps = ['100 1', '200 100', '300 200', '301 200', '400 1', ''].join('\n');
    expect(descendantPids(ps, 100)).toEqual([300, 301, 200]);
  });

  it('is empty when the root has no children', () => {
    expect(descendantPids('100 1\n', 100)).toEqual([]);
  });

  it('skips lines that are not pid pairs', () => {
    expect(descendantPids('  PID  PPID\n200 100\n', 100)).toEqual([200]);
  });
});
