import { searchParams } from 'sv-router';

/**
 * One query parameter of the current URL, for state that should survive a reload and answer
 * Android Back (a Task open in its sheet: `/todo?task=‹id›`). Reads are reactive. A change pushes a
 * history entry, so Back undoes it, unless `replace` swaps the current entry instead.
 */
export const urlParam = {
  get(name: string): string | null {
    const value = searchParams.get(name);
    return value === null ? null : String(value);
  },
  set(name: string, value: string, replace = false): void {
    searchParams.set(name, value, { replace });
  },
  delete(name: string, replace = false): void {
    searchParams.delete(name, undefined, { replace });
  },
};
