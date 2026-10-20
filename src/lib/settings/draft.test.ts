import { describe, expect, it } from 'vitest';
import { SettingsDraft } from './draft.svelte';

const saved = {
  'core.theme': 'system',
  'core.density': 'comfortable',
  'core.phone_tabs': ['home'],
};

describe('SettingsDraft', () => {
  it('starts clean and reads saved values', () => {
    const d = new SettingsDraft(saved);
    expect(d.dirty).toBe(false);
    expect(d.get('core.theme')).toBe('system');
  });

  it('is dirty after an edit and reports only the changed keys', () => {
    const d = new SettingsDraft(saved);
    d.set('core.theme', 'dark');
    expect(d.dirty).toBe(true);
    expect(d.get('core.theme')).toBe('dark');
    expect(d.changes()).toEqual([{ key: 'core.theme', value: 'dark' }]);
  });

  it('editing back to the saved value is not dirty', () => {
    const d = new SettingsDraft(saved);
    d.set('core.theme', 'dark');
    d.set('core.theme', 'system');
    expect(d.dirty).toBe(false);
    expect(d.changes()).toEqual([]);
  });

  it('compares arrays by value', () => {
    const d = new SettingsDraft(saved);
    d.set('core.phone_tabs', ['home']);
    expect(d.dirty).toBe(false);
    d.set('core.phone_tabs', ['home', 'settings']);
    expect(d.dirty).toBe(true);
  });

  it('discard reverts to saved', () => {
    const d = new SettingsDraft(saved);
    d.set('core.density', 'compact');
    d.discard();
    expect(d.dirty).toBe(false);
    expect(d.get('core.density')).toBe('comfortable');
  });

  it('rebase keeps unrelated edits and drops ones now equal to saved', () => {
    const d = new SettingsDraft(saved);
    d.set('core.theme', 'dark');
    d.set('core.density', 'compact');
    d.rebase({ ...saved, 'core.theme': 'dark' });
    expect(d.changes()).toEqual([{ key: 'core.density', value: 'compact' }]);
  });

  it('merged applies edits over saved values', () => {
    const d = new SettingsDraft(saved);
    d.set('core.theme', 'oled');
    expect(d.merged()).toEqual({ ...saved, 'core.theme': 'oled' });
  });

  it('commit makes edits the new saved values', () => {
    const d = new SettingsDraft(saved);
    d.set('core.theme', 'oled');
    d.commit();
    expect(d.dirty).toBe(false);
    expect(d.get('core.theme')).toBe('oled');
    d.set('core.theme', 'system');
    expect(d.changes()).toEqual([{ key: 'core.theme', value: 'system' }]);
  });
});
