/**
 * Browser-only stand-in for the Rust side, so the whole UI can be exercised at
 * http://localhost:47821 without Tauri. Loaded by a dynamic import guarded by
 * `import.meta.env.DEV`, so it never reaches a production build.
 * Settings validation mirrors crates/core/src/module.rs.
 *
 * Sync: a simulated second device, "Pixel 8 (simulated)" (mock-sync-pixel.ts), holds its own copy of
 * the data, makes edits and syncs with this mock's Todo, Journal and Preferences through a
 * TypeScript copy of the merge rules (mock-sync-merge.ts). That copy is guarded by
 * fixtures/sync-merge-rules.json, which src-tauri/src/sync_merge_fixture_tests.rs runs on two real
 * devices and mock-sync-merge.test.ts runs on the copy: change a rule in Rust and the Vitest case
 * fails until the copy follows. Dev only (this file is never in a production build).
 *   URL: `?sync=on` Sync on, nobody paired; `?paired=2` on with two paired devices (implies on);
 *        `&paused=update|me|both|clock|clock_me`; `&activity=N` lines; `&paircode=SECONDS`.
 *   Console: `__pixel.edit(op, args)`, `__pixel.sync()`, `__pixel.scenario(name)`, `__pixel.scenarios()`,
 *        `__pixel.world()`; `__syncMock.*` (pairing flags, setPeer, check, newConflict, memberTypedAddress);
 *        `__listMergeMock(name?)`; `__journalRemoteMock(text, id?)`.
 *
 * Every Sync scenario of docs/design/sync-prototype.html (its SCENARIOS list) and how to reach it
 * (P = `?paired=2`, which also turns Sync on):
 *   off          /settings                         Settings › Modules, Sync still off (the default)
 *   welcome      /sync?sync=on                     Sync on, no device paired: the Welcome card
 *   settings     /settings?sync=on                 the Sync section (add &paired=2 for Leave)
 *   turnoff      /settings?P                       switch Sync off: the dialog
 *   home         /?P                               Home card + sidebar badge (the 2 sample Conflicts)
 *   more         /?P in a window under 640 px      More sheet on the phone layout
 *   palette      /?P, then ⌘K and type "sync"
 *   devices      /sync?P                           add `__syncMock.setPeer('syncing')` for a row mid-sync
 *   detail       /sync?P&device=mock-pixel         the device sheet
 *   paused       /sync?P&paused=update             (other reasons: me, both, clock, clock_me)
 *   forget       /sync?P&device=mock-pixel         Forget in the sheet
 *   pairShow     /sync?sync=on, Pair a device      the phone types the code after a few seconds
 *   pairPhone    /sync?sync=on, Pair a device › Enter a code instead
 *   pairEnter    …Enter a code, type 12345678      (magic codes: mock-sync-pairing.ts header)
 *   pairAllow    …Pair a device and wait           Allow / Deny appears (or type 12345678)
 *   pairAddr     …Enter a code › Pair by address…  (`nowhere`, `closed` fail)
 *   pairPreview  …type 92000000, or `__syncMock.pixel = 'merge'` while showing a code
 *   pairDone     …type 12345678; 90000000 name clash; 91000000 joins a group
 *   pairError    …type 11111111 (wrong code); 20000000 expired; 30000000 declined
 *   conflicts    /sync/conflicts?P                 2 sample Conflicts and 5 Resolved rows; real ones
 *                                                   from `__pixel.scenario('conflict')` and 'resolved'
 *   compare      /sync/conflicts?P, open a row     (Compare)
 *   merged       Compare › Edit merged
 *   activity     /sync/activity?P                  4000 lines; `__pixel.sync()` adds new ones
 *   remote       /todo?P on Today, then `__pixel.scenario('remote')`    "Call mum" changes under you
 *   taskSheet    `__pixel.scenario('conflict')`, then open the URL it prints (/todo?task=ID)
 *   remoteDelete `__pixel.scenario('remoteDelete')`, open the Task "Call mum", then `__pixel.sync()`
 *   listMerged   `__pixel.scenario('listMerged')`, open the List "Groceries", then `__pixel.sync()`
 *                (or `__listMergeMock('Groceries')`)
 */
import { mockIPC } from '@tauri-apps/api/mocks';
import type { AppError, DeviceSettings, JsonValue, SettingsChange } from './bindings';
import { events } from './bindings';
import { createJournalMock } from './mock-journal';
import { createSyncMock } from './mock-sync';
import { createActivityMock } from './mock-sync-activity';
import { createConflictsMock } from './mock-sync-conflicts';
import type { Json } from './mock-sync-merge';
import { defaultPairingFlags, type PairingFlags } from './mock-sync-pairing';
import { createPixelMock } from './mock-sync-pixel';
import { createTodoMock } from './mock-todo';
import { PREF_VALUES } from './mock-todo-arrange';

type Validator = (v: JsonValue) => string | null;

const oneOf =
  (...allowed: string[]): Validator =>
  (v) =>
    typeof v === 'string' && allowed.includes(v) ? null : `must be one of ${allowed.join(', ')}`;
const bool: Validator = (v) => (typeof v === 'boolean' ? null : 'must be true or false');
const text =
  (max: number): Validator =>
  (v) =>
    typeof v === 'string' && [...v].length <= max ? null : `must be text up to ${max} characters`;
const list =
  (max: number): Validator =>
  (v) =>
    Array.isArray(v) && v.length <= max && v.every((x) => typeof x === 'string')
      ? null
      : `must be a list of up to ${max} strings`;

const DEFS: Record<string, { default: JsonValue; validate: Validator }> = {
  'core.theme': { default: 'system', validate: oneOf('system', 'light', 'dark', 'oled') },
  'core.density': { default: 'comfortable', validate: oneOf('comfortable', 'compact') },
  'core.text_size': { default: 'm', validate: oneOf('s', 'm', 'l') },
  'core.reduce_motion': { default: false, validate: bool },
  'core.display_name': { default: '', validate: text(60) },
  'core.time_format': { default: '24h', validate: oneOf('24h', '12h') },
  'core.date_format': { default: 'long', validate: oneOf('long', 'short', 'iso') },
  'core.week_start': { default: 'mon', validate: oneOf('mon', 'sun') },
  'core.phone_tabs': { default: ['home', 'journal', 'todo'], validate: list(4) },
  'core.phone_tabs_edited': { default: false, validate: bool },
  'core.disabled_modules': { default: [], validate: list(64) },
  'core.enabled_optional_modules': { default: [], validate: list(64) },
  'journal.font': { default: 'serif', validate: oneOf('serif', 'sans') },
  'journal.width': { default: 'default', validate: oneOf('narrow', 'default', 'wide') },
  'journal.page_summary': { default: true, validate: bool },
  'todo.parse_dates': { default: true, validate: bool },
  // How the views group and sort, and what Done Tasks do in a List (crates/todo/src/arrange.rs).
  'todo.group_today': { default: 'priority', validate: oneOf(...PREF_VALUES.group_today) },
  'todo.group_upcoming': { default: 'day', validate: oneOf(...PREF_VALUES.group_upcoming) },
  'todo.group_inbox': { default: 'none', validate: oneOf(...PREF_VALUES.group_inbox) },
  'todo.group_all': { default: 'list', validate: oneOf(...PREF_VALUES.group_all) },
  'todo.inbox_sort': { default: 'manual', validate: oneOf(...PREF_VALUES.inbox_sort) },
  'todo.lists_sort': { default: 'manual', validate: oneOf(...PREF_VALUES.lists_sort) },
  'todo.done_in_lists': { default: 'today_shown', validate: oneOf(...PREF_VALUES.done_in_lists) },
};

/** Settings kept in the device file (crates/core/src/device.rs `LOOK_KEYS`). */
const LOOK_KEYS: string[] = [
  'core.theme',
  'core.density',
  'core.text_size',
  'core.reduce_motion',
  'core.phone_tabs',
];

const err = (code: AppError['code'], message: string): AppError => ({
  code,
  message,
  details: null,
});

export async function installMock() {
  const values: Record<string, JsonValue> = Object.fromEntries(
    Object.entries(DEFS).map(([k, d]) => [k, d.default]),
  );
  const device: DeviceSettings = {
    sidebar: { width: 248, rail: false, todo_open: null },
    log_level: 'info',
    spellcheck: true,
    look: {},
    sync: { enabled: false, name: null, automatic: true, mobile_data: false, visible: true },
  };

  const query = new URLSearchParams(location.search);
  // `?sync=on` (or any `?paired=N`) starts with Sync turned on, as if it had been set up before.
  if (query.get('sync') === 'on' || Number(query.get('paired') ?? 0) > 0)
    device.sync = { ...device.sync, enabled: true, name: 'Alex’s MacBook Pro' };
  const journalFlags = {
    failUpdate: query.get('fail') === 'journal_update',
    failPages: query.get('fail') === 'journal_pages',
  };
  // Reachable from the dev console: `__journalMock.failUpdate = false` lets a Retry succeed.
  (window as unknown as { __journalMock: typeof journalFlags }).__journalMock = journalFlags;
  const journal = createJournalMock(query.get('seed') === 'journal', journalFlags);

  journal.onRemoteChange = () => void events.journalChangedEvent.emit();
  // Console: `__journalRemoteMock('new text')` acts out another device editing the newest Entry
  // (or the Entry with the given id). The open Journal refreshes silently; an Entry being edited
  // keeps its text until the editor closes. Browser only; Rust sends the same event after a sync.
  (window as unknown as { __journalRemoteMock: unknown }).__journalRemoteMock = (
    text: string,
    id?: string,
  ) => {
    if (journal.simulateRemoteEdit(text, id)) void events.journalChangedEvent.emit();
  };

  const todo = createTodoMock((key) => values[key]);
  // Console: `__listMergeMock()` (or `('Groceries', 'Pixel 8')`) acts out another device having made
  // a List with the same name: the one you are looking at merges away, you are moved to the
  // survivor and told. Browser only; Rust sends the same event after a real sync.
  (window as unknown as { __listMergeMock: unknown }).__listMergeMock = (
    name?: string,
    deviceName = 'Pixel 8',
  ) => {
    const merged = todo.simulateMerge(name, deviceName);
    if (!merged) return null;
    void events.todoListsMergedEvent.emit(merged);
    void events.todoChangedEvent.emit();
    return merged;
  };
  // `?paired=2` shows Sync with two paired devices (Leave enabled, no Welcome page).
  // `__syncMock` steers the simulated second device (see mock-sync-pairing.ts); `?paircode=SECONDS`
  // makes a shown code expire sooner.
  const pairFlags = defaultPairingFlags();
  const codeSeconds = Number(query.get('paircode') ?? 0);
  if (codeSeconds > 0) pairFlags.lifeMs = codeSeconds * 1000;
  (window as unknown as { __syncMock: PairingFlags }).__syncMock = pairFlags;
  // Thousands of lines to scroll: `?activity=20000` for more. It shows on the Activity tab with a
  // paired device (`?paired=2`).
  // The Review link on an Activity line follows the Conflict: gone once it is resolved.
  const activity = createActivityMock(
    Number(query.get('activity') ?? 0) || 4000,
    Date.now(),
    (id) => conflicts.isOpen(id),
  );
  // The sample Conflicts and Resolved rows exist only once a device is paired.
  const conflicts = createConflictsMock(
    () => void events.syncConflictsEvent.emit(),
    Date.now,
    () => sync.pairedCount() > 0,
  );
  const sync = createSyncMock(
    device,
    Number(query.get('paired') ?? 0) || 0,
    (view) => void events.syncPairingEvent.emit(view),
    pairFlags,
    () => void events.syncPeersEvent.emit(),
    conflicts.openCount,
  );
  // `?paired=2&paused=update|me|both|clock|clock_me` starts with Pixel 8 paused for that reason.
  const paused = query.get('paused');
  if (paused) pairFlags.setPeer?.('paused', undefined, 0, paused);
  // Console: `__syncMock.newConflict()` acts out a conflict arriving; a toast shows if Settings ›
  // Sync is on "Show a toast" (Rust only sends the event to a device set to one).
  pairFlags.newConflict = () => {
    const wants = (device.sync as { toast_conflicts?: boolean } | undefined)?.toast_conflicts;
    if (wants) void events.syncNewConflictsEvent.emit({ count: 1, device: 'Pixel 8 (simulated)' });
  };

  // The simulated second device: `__pixel.edit(…)`, `__pixel.sync()`, `__pixel.scenario(…)`
  // (mock-sync-pixel.ts). It syncs with this mock's Todo, Journal and Preferences through the TS
  // merge (mock-sync-merge.ts).
  const touchedPrefs = new Set<string>();
  const pixel = createPixelMock({
    data: { todo, journal },
    settings: {
      // A Preference is part of the data once it differs from its default (or did before).
      exportDocs: () =>
        Object.entries(values)
          .filter(([k, v]) => {
            if (LOOK_KEYS.includes(k) || !(k in DEFS)) return false;
            if (JSON.stringify(v) !== JSON.stringify(DEFS[k]?.default)) touchedPrefs.add(k);
            return touchedPrefs.has(k);
          })
          .map(([k, v]) => ({
            entity: 'setting' as const,
            id: k,
            values: { value: v as unknown as Json },
          })),
      importDocs: (docs) => {
        const changed: string[] = [];
        for (const d of docs) {
          const def = DEFS[d.id];
          const value = d.values.value as JsonValue;
          if (!def || def.validate(value) !== null) continue;
          touchedPrefs.add(d.id);
          if (JSON.stringify(values[d.id]) !== JSON.stringify(value)) {
            values[d.id] = value;
            changed.push(d.id);
          }
        }
        return changed;
      },
    },
    command: (cmd, a) => (todo.handles(cmd) ? todo.handle(cmd, a) : journal.handle(cmd, a)),
    conflicts,
    activity,
    myName: () => (device.sync?.name as string | null) ?? 'Alex’s MacBook Pro',
    announce: {
      todoChanged: () => void events.todoChangedEvent.emit(),
      journalChanged: () => void events.journalChangedEvent.emit(),
      listsMerged: (e) => void events.todoListsMergedEvent.emit(e),
      taskDeleted: (e) => void events.todoTaskDeletedEvent.emit(e),
      activityChanged: () => void events.syncActivityEvent.emit(),
      settingsChanged: (module, keys) => void events.settingsChangedEvent.emit({ module, keys }),
      newConflicts: (count, name) => {
        if ((device.sync as { toast_conflicts?: boolean } | undefined)?.toast_conflicts)
          void events.syncNewConflictsEvent.emit({ count, device: name });
      },
    },
  });
  (window as unknown as { __pixel: unknown }).__pixel = pixel;

  applyPrePaintAttributes(values);

  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      switch (cmd) {
        case 'settings_get_all':
          return {
            // A JSON copy, like real IPC: handing out the live object hides later saves from the query cache.
            values: JSON.parse(JSON.stringify(values)),
            schema: Object.entries(DEFS).map(([key, d]) => ({
              key,
              module: key.split('.')[0],
              default: d.default,
            })),
          };
        case 'settings_save': {
          const changes = a.changes as SettingsChange[];
          for (const c of changes) {
            const def = DEFS[c.key];
            if (!def) throw err('unknown_key', `Unknown setting: ${c.key}`);
            const problem = def.validate(c.value);
            if (problem) throw err('validation', `${c.key} ${problem}`);
          }
          for (const c of changes) values[c.key] = JSON.parse(JSON.stringify(c.value));
          // Saving the tabs marks them as edited, like Rust.
          if (changes.some((c) => c.key === 'core.phone_tabs'))
            values['core.phone_tabs_edited'] = true;
          // Look and feel is a Device setting: Rust keeps it in device.toml, not the database.
          const look = device.look as Record<string, JsonValue>;
          for (const c of changes)
            if (LOOK_KEYS.includes(c.key)) look[c.key.slice('core.'.length)] = c.value;
          if (changes.some((c) => c.key === 'core.phone_tabs')) look.phone_tabs_edited = true;
          // Same shape the Rust side emits: one event per module, keys without the module prefix.
          for (const module of new Set(changes.map((c) => c.key.split('.')[0] as string))) {
            void events.settingsChangedEvent.emit({
              module,
              keys: changes
                .filter((c) => c.key.startsWith(`${module}.`))
                .map((c) => c.key.slice(module.length + 1)),
            });
          }
          return null;
        }
        case 'device_get':
          // A JSON copy, like real IPC.
          return JSON.parse(JSON.stringify(device));
        case 'device_set_sidebar':
          device.sidebar = {
            ...device.sidebar,
            width: Math.min(360, Math.max(200, a.width as number)),
            rail: a.rail as boolean,
          };
          return null;
        case 'device_set_todo_open':
          device.sidebar = { ...device.sidebar, todo_open: a.open === true };
          return null;
        case 'device_set_log_level': {
          const level = String(a.level).toLowerCase();
          if (!['error', 'warn', 'info', 'debug', 'trace'].includes(level)) {
            throw err('validation', `Unknown log level: ${level}`);
          }
          device.log_level = level;
          return null;
        }
        case 'device_set_spellcheck':
          device.spellcheck = a.enabled === true;
          return null;
        case 'app_info':
          return {
            version: '0.1.0',
            environment: 'sandbox',
            data_dir: '/mock/Mnemora/sandbox',
            log_dir: '/mock/Mnemora/sandbox/logs',
            os_first_name: 'Alex',
          };
        case 'app_take_links':
          return [];
        case 'log_ui_error':
          console.warn('[mock] logUiError', a.message);
          return null;
        case 'backup_now':
          return '/mock/Mnemora/sandbox/backups/mnemora-backup.db';
        case 'open_log_dir':
          return null;
        default:
          if (activity.handles(cmd)) return activity.handle(cmd, a);
          if (conflicts.handles(cmd)) return conflicts.handle(cmd, a);
          if (sync.handles(cmd)) return sync.handle(cmd, a);
          if (todo.handles(cmd)) return todo.handle(cmd, a);
          return journal.handles(cmd) ? journal.handle(cmd, a) : undefined;
      }
    },
    { shouldMockEvents: true },
  );
}

/** What Rust's window-init script does before first paint. */
function applyPrePaintAttributes(values: Record<string, JsonValue>) {
  const d = document.documentElement.dataset;
  const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
  d.theme = dark ? 'dark' : 'light';
  d.themePref = String(values['core.theme']);
  d.density = String(values['core.density']);
  d.textSize = String(values['core.text_size']);
  d.reduceMotion = String(values['core.reduce_motion']);
  d.env = 'sandbox';
}
