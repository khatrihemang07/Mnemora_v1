import type { ActivityFilter, ActivityRow, DevicesView } from '../../lib/ipc/bindings';
import { groupByDay } from './conflicts';

/** The chip choosing "All": the value `FilterChips` holds. */
export const CHIP_ALL = 'all';
/** The chip choosing "This device". */
export const CHIP_THIS = 'this';

/** What a chip asks Rust for: a device chip carries the device id. */
export function filterOf(chip: string): ActivityFilter {
  if (chip === CHIP_ALL) return { kind: 'all' };
  if (chip === CHIP_THIS) return { kind: 'this_device' };
  return { kind: 'device', id: chip };
}

/** All · This device · each paired device, in the order the Devices tab lists them. */
export function chipsOf(peers: Pick<DevicesView['peers'][number], 'id' | 'name'>[]) {
  return [
    { value: CHIP_ALL, label: 'All' },
    { value: CHIP_THIS, label: 'This device' },
    ...peers.map((p) => ({ value: p.id, label: p.name })),
  ];
}

/** A chip whose device was forgotten falls back to All. */
export function chipStillThere(chip: string, peers: { id: string }[]): string {
  return chip === CHIP_ALL || chip === CHIP_THIS || peers.some((p) => p.id === chip)
    ? chip
    : CHIP_ALL;
}

/** One line of the virtual list: a day's heading, or a row of the log. */
export type ActivityItem =
  | { kind: 'day'; key: string; title: string; subtitle: string; first: boolean }
  | { kind: 'row'; key: string; row: ActivityRow };

/** The rows (newest first) as heading + rows per local day, flattened for the virtual list. */
export function itemsOf(rows: ActivityRow[], now: Date = new Date()): ActivityItem[] {
  const items: ActivityItem[] = [];
  for (const group of groupByDay(rows, now, (r) => r.at)) {
    // As Done has it: Today, Yesterday, then the whole weekday ("Monday") with the date beside it.
    const recent = group.title === 'Today' || group.title === 'Yesterday';
    items.push({
      kind: 'day',
      key: `day:${group.key}`,
      title: recent ? group.title : (group.subtitle.split(' ')[0] ?? group.title),
      subtitle: group.subtitle,
      first: items.length === 0,
    });
    for (const row of group.rows) items.push({ kind: 'row', key: row.id, row });
  }
  return items;
}

/** A first guess of an item's height in px (rows are measured once shown). */
export function estimate(item: ActivityItem, open: boolean): number {
  if (item.kind === 'day') return item.first ? 36 : 58;
  return item.row.breakdown && open ? 78 : 52;
}

/** The sentence of a row, as plain text. */
export function lineText(row: ActivityRow): string {
  const text = row.text.map((p) => p.text).join('');
  return row.on ? `${text} · on ${row.on}` : text;
}

/** Whether a row opens into a breakdown. */
export const expandable = (row: ActivityRow): boolean => row.breakdown !== null;

/** Scrolled this near the top (px) the list takes in new lines at once instead of waiting. */
export const TOP_PX = 400;
