import { describe, expect, it } from 'vitest';
import type { ModuleContext, ModuleManifest } from '../lib/router';
import {
  bareKeyFor,
  enabledModules,
  homeCards,
  moduleSettingsSections,
  overlayModules,
  redirectForDisabled,
  setModuleEnabled,
  setOptedIn,
} from './enabled-modules';

const comp = async () => ({ default: (() => {}) as never });
const mod = (id: string, order: number, extra: Partial<ModuleManifest> = {}): ModuleManifest => ({
  id,
  name: id,
  icon: {} as never,
  accentVar: `--hue-${id}`,
  routes: [{ path: `/${id}`, component: comp }],
  nav: { order },
  ...extra,
});

const home = mod('home', 10, { required: true, routes: [{ path: '/', component: comp }] });
const settings = mod('settings', 100, { required: true });
const journal = mod('journal', 20, {
  homeCard: comp,
  settingsSection: { label: 'Journal', icon: {} as never, component: comp },
});
const todo = mod('todo', 30, { homeCard: comp });
const all = [settings, todo, home, journal];
const ids = (ms: { id: string }[]) => ms.map((m) => m.id);

describe('enabledModules', () => {
  it('keeps every Module when nothing is disabled', () => {
    expect(ids(enabledModules(all, []))).toEqual(['settings', 'todo', 'home', 'journal']);
  });
  it('drops disabled Modules', () => {
    expect(ids(enabledModules(all, ['journal']))).toEqual(['settings', 'todo', 'home']);
  });
  it('never drops a required Module', () => {
    expect(ids(enabledModules(all, ['home', 'settings']))).toEqual([
      'settings',
      'todo',
      'home',
      'journal',
    ]);
  });
  it('ignores ids of Modules that no longer exist', () => {
    expect(ids(enabledModules(all, ['gone']))).toHaveLength(4);
  });
});

describe('setModuleEnabled', () => {
  it('adds a Module to the disabled list when switched off', () => {
    expect(setModuleEnabled([], 'journal', false)).toEqual(['journal']);
  });
  it('removes it when switched on', () => {
    expect(setModuleEnabled(['journal', 'todo'], 'journal', true)).toEqual(['todo']);
  });
  it('does not duplicate', () => {
    expect(setModuleEnabled(['journal'], 'journal', false)).toEqual(['journal']);
  });
});

describe('homeCards', () => {
  it('lists cards of Modules that have one, in nav order', () => {
    expect(homeCards(all).map((c) => c.id)).toEqual(['journal', 'todo']);
  });
  it('omits cards of disabled Modules', () => {
    expect(homeCards(enabledModules(all, ['journal'])).map((c) => c.id)).toEqual(['todo']);
  });
});

describe('moduleSettingsSections', () => {
  it('returns the sections of enabled Modules that have one', () => {
    const out = moduleSettingsSections(all);
    expect(out.map((s) => [s.id, s.label])).toEqual([['journal', 'Journal']]);
  });
  it('omits sections of disabled Modules', () => {
    expect(moduleSettingsSections(enabledModules(all, ['journal']))).toEqual([]);
  });
});

describe('redirectForDisabled', () => {
  const enabled = enabledModules(all, ['journal']);
  it('sends a disabled Module route to Home', () => {
    expect(redirectForDisabled('/journal', all, enabled)).toBe('/');
  });
  it('covers sub-paths of a disabled Module', () => {
    expect(redirectForDisabled('/journal/2026-01-01', all, enabled)).toBe('/');
  });
  it('leaves enabled and unknown routes alone', () => {
    expect(redirectForDisabled('/todo', all, enabled)).toBeNull();
    expect(redirectForDisabled('/nowhere', all, enabled)).toBeNull();
  });
});

describe('bareKeyFor', () => {
  const ctx = {} as ModuleContext;
  const run = () => {};
  const keyed = mod('todo', 30, { bareKeys: [{ key: 'q', run }] });
  const other = mod('journal', 20, {
    bareKeys: [
      { key: 'n', run },
      { key: 'q', run: () => {} },
    ],
  });

  it('finds the Module that owns a key, in either case', () => {
    expect(bareKeyFor([keyed], 'q', ctx)?.run).toBe(run);
    expect(bareKeyFor([keyed], 'Q', ctx)?.run).toBe(run);
    expect(bareKeyFor([keyed], 'n', ctx)).toBeNull();
  });

  it('gives a contested key to the first Module by nav order', () => {
    expect(bareKeyFor([keyed, other], 'q', ctx)?.run).not.toBe(run);
    expect(bareKeyFor([keyed, other], 'n', ctx)).not.toBeNull();
  });

  it('forgets a Module that is switched off', () => {
    expect(bareKeyFor(enabledModules([keyed, other], ['journal']), 'q', ctx)?.run).toBe(run);
  });

  it('skips a claim whose when does not hold, and asks it with the context', () => {
    const onlyOnTodo = mod('todo', 20, {
      bareKeys: [{ key: '/', when: (c) => c === ctx, run }],
    });
    const fallback = mod('journal', 30, { bareKeys: [{ key: '/', run: () => {} }] });
    expect(bareKeyFor([onlyOnTodo, fallback], '/', ctx)?.run).toBe(run);
    expect(bareKeyFor([onlyOnTodo, fallback], '/', {} as ModuleContext)?.run).not.toBe(run);
    expect(bareKeyFor([onlyOnTodo], '/', {} as ModuleContext)).toBeNull();
  });
});

describe('overlayModules', () => {
  it('lists the enabled Modules that mount an overlay, in nav order', () => {
    const a = mod('a', 40, { overlay: comp });
    const b = mod('b', 25, { overlay: comp });
    expect(ids(overlayModules([a, journal, b]))).toEqual(['b', 'a']);
  });
});

describe('default-off Modules', () => {
  const sync = mod('sync', 90, { defaultEnabled: false });
  const withSync = [...all, sync];

  it('are off until opted in', () => {
    expect(ids(enabledModules(withSync, []))).not.toContain('sync');
    expect(ids(enabledModules(withSync, [], ['sync']))).toContain('sync');
  });
  it('ignore the disabled list: only an opt-in turns them on', () => {
    expect(ids(enabledModules(withSync, ['sync'], ['sync']))).toContain('sync');
  });
  it('do not change a default-on Module, whatever is opted in', () => {
    expect(ids(enabledModules(withSync, ['todo'], ['todo']))).not.toContain('todo');
    expect(ids(enabledModules(withSync, [], ['todo']))).toContain('todo');
  });
  it('leave the opted-in list as a plain set of ids', () => {
    expect(setOptedIn([], 'sync', true)).toEqual(['sync']);
    expect(setOptedIn(['sync'], 'sync', true)).toEqual(['sync']);
    expect(setOptedIn(['sync', 'x'], 'sync', false)).toEqual(['x']);
  });
  it('send a switched-off Module URL home', () => {
    const everything = [home, settings, sync];
    expect(redirectForDisabled('/sync/activity', everything, enabledModules(everything, []))).toBe(
      '/',
    );
    expect(
      redirectForDisabled('/sync/activity', everything, enabledModules(everything, [], ['sync'])),
    ).toBeNull();
  });
});
