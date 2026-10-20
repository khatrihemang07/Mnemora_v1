import { describe, expect, it } from 'vitest';
import type { ModuleManifest } from '../lib/router';
import {
  createTarget,
  enabledModules,
  homeCards,
  moduleSettingsSections,
  redirectForDisabled,
  setModuleEnabled,
} from './enabled-modules';

const comp = async () => ({ default: (() => {}) as never });
const mod = (id: string, order: number, extra: Partial<ModuleManifest> = {}): ModuleManifest => ({
  id,
  name: id,
  icon: {} as never,
  accentVar: `--hue-${id}`,
  routes: [{ path: `/${id}`, component: comp }],
  nav: { order, shortcut: null },
  ...extra,
});

const home = mod('home', 10, { required: true, routes: [{ path: '/', component: comp }] });
const settings = mod('settings', 100, { required: true });
const journal = mod('journal', 20, {
  homeCard: comp,
  create: () => {},
  settingsSection: { label: 'Journal', icon: {} as never, component: comp },
});
const todo = mod('todo', 30, { homeCard: comp, create: () => {} });
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

describe('createTarget', () => {
  it('picks the first Module with a create action by nav order', () => {
    expect(createTarget(all)?.id).toBe('journal');
  });
  it('skips Modules that are off', () => {
    expect(createTarget(enabledModules(all, ['journal']))?.id).toBe('todo');
  });
  it('is undefined when no enabled Module can create', () => {
    expect(createTarget(enabledModules(all, ['journal', 'todo']))).toBeUndefined();
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
