/**
 * Browser-only stand-in for the Rust side, so the whole UI can be exercised at
 * http://localhost:47821 without Tauri. Loaded by a dynamic import guarded by
 * `import.meta.env.DEV`, so it never reaches a production build.
 * Settings validation mirrors crates/core/src/module.rs.
 */
import { mockIPC } from '@tauri-apps/api/mocks';
import type { AppError, DeviceSettings, JsonValue, SettingsChange } from './bindings';
import { events } from './bindings';
import { createJournalMock } from './mock-journal';
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
  'core.phone_tabs': { default: ['home', 'journal'], validate: list(4) },
  'core.disabled_modules': { default: [], validate: list(64) },
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
  };

  const query = new URLSearchParams(location.search);
  const journalFlags = {
    failUpdate: query.get('fail') === 'journal_update',
    failPages: query.get('fail') === 'journal_pages',
  };
  // Reachable from the dev console: `__journalMock.failUpdate = false` lets a Retry succeed.
  (window as unknown as { __journalMock: typeof journalFlags }).__journalMock = journalFlags;
  const journal = createJournalMock(query.get('seed') === 'journal', journalFlags);

  const todo = createTodoMock((key) => values[key]);

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
        case 'log_ui_error':
          console.warn('[mock] logUiError', a.message);
          return null;
        case 'backup_now':
          return '/mock/Mnemora/sandbox/backups/mnemora-backup.db';
        case 'open_log_dir':
          return null;
        default:
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
