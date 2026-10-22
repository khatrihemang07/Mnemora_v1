import { describe, expect, it } from 'vitest';
import { claimTabFocus, HANDOFF_MS, noteTabChange } from './tab-handoff';

describe('tab focus handoff', () => {
  it('hands focus to the next Tabs that mounts after a change, once', () => {
    noteTabChange(1000);
    expect(claimTabFocus(1000 + 50)).toBe(true);
    expect(claimTabFocus(1000 + 60)).toBe(false);
  });

  it('does nothing for a mount with no recent change', () => {
    expect(claimTabFocus(5000)).toBe(false);
  });

  it('forgets a change that nothing mounted after', () => {
    noteTabChange(1000);
    expect(claimTabFocus(1000 + HANDOFF_MS + 1)).toBe(false);
  });
});
