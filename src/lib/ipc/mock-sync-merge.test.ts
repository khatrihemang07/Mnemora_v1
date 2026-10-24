import { describe, expect, it } from 'vitest';
import rules from './fixtures/sync-merge-rules.json';
import { LOOK_KEYS, textMerge, textOf } from './mock-sync-merge';
import {
  createIds,
  createReplica,
  perform,
  type Step,
  syncReplicas,
  worldOf,
} from './mock-sync-replica';

const START = 1_800_000_000_000;
const DEFAULTS = { 'core.theme': 'system' };

interface Case {
  name: string;
  steps: Step[];
  expect: Record<string, Record<string, unknown> | unknown>;
}

/** Runs one fixture case on two simulated devices (src-tauri/src/sync_merge_fixture_tests.rs does the same on two real ones). */
function run(c: Case) {
  const devices = { a: createReplica('a'), b: createReplica('b') };
  const ids = createIds();
  c.steps.forEach((s, n) => {
    if (s.op === 'sync') syncReplicas(devices.a, devices.b);
    else perform(devices[s.d === 'b' ? 'b' : 'a'], s, ids, START + (n + 1) * 1000);
  });
  syncReplicas(devices.a, devices.b);
  return {
    a: worldOf(devices.a, ids, DEFAULTS),
    b: worldOf(devices.b, ids, DEFAULTS),
  };
}

const shared = (w: ReturnType<typeof run>['a']) => ({
  ...w,
  settings: Object.fromEntries(Object.entries(w.settings).filter(([k]) => !LOOK_KEYS.includes(k))),
});

describe('the mock’s TypeScript merge against the shared fixture', () => {
  for (const c of rules.cases as unknown as Case[]) {
    it(c.name, () => {
      const worlds = run(c);
      expect(shared(worlds.a), 'the devices agree').toEqual(shared(worlds.b));
      const at = (o: unknown, ...path: string[]) =>
        path.reduce((x, p) => (x as Record<string, unknown> | undefined)?.[p], o);
      for (const [key, want] of Object.entries(c.expect)) {
        for (const d of ['a', 'b'] as const) {
          const w = worlds[d];
          const each = Object.entries(want as Record<string, unknown>);
          if (key === 'device') {
            for (const [setting, by] of each)
              expect(at(w, 'settings', setting), `${setting} on ${d}`).toEqual(at(by, d));
          } else if (key === 'tasks' || key === 'comments') {
            for (const [label, fields] of each)
              for (const [f, value] of Object.entries(fields as Record<string, unknown>))
                expect(at(w, key, label, f), `${key} ${label}.${f} on ${d}`).toEqual(value);
          } else if (key === 'settings' || key === 'order') {
            for (const [item, value] of each)
              expect(at(w, key, item), `${key} ${item} on ${d}`).toEqual(value);
          } else expect(at(w, key), `${key} on ${d}`).toEqual(want);
        }
      }
    });
  }
});

describe('text merge', () => {
  it('shows the newer side of a clash and keeps the words only one side changed', () => {
    const pieces = textMerge('a b c', 'a x c', 'a y c d');
    expect(textOf(pieces)).toBe('a y c d');
    expect(pieces.some((p) => 'clash' in p)).toBe(true);
  });

  it('settles a change inside the other on the longer one', () => {
    expect(textOf(textMerge('one two', 'one two three', 'one two three four'))).toBe(
      'one two three four',
    );
  });
});
