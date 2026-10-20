import { clockParts, type DateFormat, formatDate, type TimeFormat } from '../../lib/time/format';
import type { SlashValues } from '../../lib/ui/markdown/slash';

/** Today's date and the current time in the General Settings formats, for the slash menu. */
export function slashValues(
  now: Date,
  dateFormat: DateFormat,
  timeFormat: TimeFormat,
): SlashValues {
  const { hm, period } = clockParts(now, timeFormat);
  return { date: formatDate(now, dateFormat), time: period ? `${hm} ${period}` : hm };
}
