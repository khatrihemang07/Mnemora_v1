import type { JsonValue } from '../../lib/ipc/bindings';

export type JournalFont = 'serif' | 'sans';
export type JournalWidth = 'narrow' | 'default' | 'wide';

/** Typed view of the `journal.*` Preferences. Rust validates; this only narrows and fills fallbacks. */
export interface JournalSettings {
  font: JournalFont;
  width: JournalWidth;
  /** The "N entries · N words" line in Page headers. */
  pageSummary: boolean;
}

/**
 * Column width per setting, in rem so it scales with Text size: about 60, 72 and 90 characters of
 * the body font. The column shrinks below this to fill a narrow window.
 */
export const WIDTH_REM: Record<JournalWidth, number> = { narrow: 35.5, default: 42.5, wide: 53 };

/** The CSS length of the text column for a width setting. */
export const columnWidth = (width: JournalWidth): string => `${WIDTH_REM[width]}rem`;

const FONTS: readonly JournalFont[] = ['serif', 'sans'];
const WIDTHS: readonly JournalWidth[] = ['narrow', 'default', 'wide'];

export function readJournal(values: Record<string, JsonValue> = {}): JournalSettings {
  const font = values['journal.font'];
  const width = values['journal.width'];
  return {
    font: FONTS.includes(font as JournalFont) ? (font as JournalFont) : 'serif',
    width: WIDTHS.includes(width as JournalWidth) ? (width as JournalWidth) : 'default',
    pageSummary: values['journal.page_summary'] !== false,
  };
}
