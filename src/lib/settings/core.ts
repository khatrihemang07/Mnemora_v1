import type { JsonValue } from '../ipc/bindings';
import type { ThemePref } from '../theme/resolve';
import type { DateFormat, TimeFormat } from '../time/format';

export type Density = 'comfortable' | 'compact';
export type TextSize = 's' | 'm' | 'l';

/** Typed view of the `core.*` Settings. Rust validates; this only narrows and fills fallbacks. */
export interface CoreSettings {
  theme: ThemePref;
  density: Density;
  textSize: TextSize;
  reduceMotion: boolean;
  displayName: string;
  timeFormat: TimeFormat;
  dateFormat: DateFormat;
  weekStart: 'mon' | 'sun';
  phoneTabs: string[];
  /** Modules switched off; absent Modules are on, so new ones appear by default. */
  disabledModules: string[];
}

type Values = Record<string, JsonValue>;

const pick = <T extends string>(v: JsonValue | undefined, allowed: readonly T[], fallback: T): T =>
  allowed.includes(v as T) ? (v as T) : fallback;
const strings = (v: JsonValue | undefined): string[] =>
  Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string') : [];

export function readCore(values: Values = {}): CoreSettings {
  return {
    theme: pick(values['core.theme'], ['system', 'light', 'dark', 'oled'], 'system'),
    density: pick(values['core.density'], ['comfortable', 'compact'], 'comfortable'),
    textSize: pick(values['core.text_size'], ['s', 'm', 'l'], 'm'),
    reduceMotion: values['core.reduce_motion'] === true,
    displayName: typeof values['core.display_name'] === 'string' ? values['core.display_name'] : '',
    timeFormat: pick(values['core.time_format'], ['24h', '12h'], '24h'),
    dateFormat: pick(values['core.date_format'], ['long', 'short', 'iso'], 'long'),
    weekStart: pick(values['core.week_start'], ['mon', 'sun'], 'mon'),
    phoneTabs: strings(values['core.phone_tabs']),
    disabledModules: strings(values['core.disabled_modules']),
  };
}
