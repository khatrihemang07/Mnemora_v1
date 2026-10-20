/**
 * Dev-only sample Journal for the browser mock, loaded by `?seed=journal`. It spreads Entries over
 * today, yesterday, a backfilled week-old day, last month, the previous year and ~60 older Pages,
 * so every feed state (badges, month dividers, years, written dates, scrolling) can be seen.
 */
import type { Entry } from './bindings';

const ZONE = 'Asia/Kolkata';
const OFFSET_MIN = 330;
const DAY_MS = 86_400_000;

const POOL = [
  'Woke up before the alarm for once. Made chai, sat on the balcony and watched the street come alive. No phone until nine.',
  'Long standup, then a quiet afternoon of actual work. Finally untangled the sync bug that had been bothering me all week.\n\nThe fix was one line. It usually is.',
  'Dinner with Mira and Dev. We talked about the trip in spring and whether three weeks is too long. (It is not.)',
  'Rain all day. Stayed in, reorganised the bookshelf, found the notebook I lost in March. The first page just says "start again".',
  'Ran 5k along the river. Slower than last month but my knee felt fine, which is the real win.',
  'Reading *The Overstory* again. One line stayed with me: trees do not so much grow as keep deciding to.',
  'Call with Mum. She is thinking about repainting the front room yellow. I voted yes.\n\nShe also sent photos of the new puppy, who has eaten two slippers already.',
  'Low energy. Did the minimum, went to bed early, no guilt about it.',
  'Idea for the weekend: take the early train to the coast, walk until lunch, come back on the slow bus.',
  'Team offsite planning. Too many meetings, but the whiteboard session at the end was good and we agreed on the three things that matter this quarter.\n\nTo do:\n- book the venue\n- send the agenda\n- ask Priya about budget',
  'Tried the new bakery on the corner. The cardamom bun is dangerous. I will be back tomorrow.',
  'Cleared my inbox down to zero for the first time in a year. It lasted about forty minutes.',
  'Slept badly. Strong coffee, a long walk, and a good conversation with a stranger at the bus stop about monsoons.',
  'Finished the quarterly review draft. Writing down what went well first made the rest easier to face.\n\nNote to self: do this every month, not every quarter.\n\nNext step is sharing it on Friday.',
];

/** One Entry using every supported Markdown feature, to check rendering and editing. */
const SHOWCASE = `# A morning plan

Some **bold**, *italic*, ~~struck~~ and ==highlighted== words, \`inline code\` and a [link](https://example.com), or just https://example.com/bare.

## Today

- [ ] Write the report
- [x] Water the plants
- plain bullet
  - nested bullet

1. First
2. Second

> A quote that wraps across the line to show the bar.

---

\`\`\`
const x = 1;
\`\`\`

| a | b |
|---|---|`;

/** An Entry that is only a checklist: ⋯ ▸ Edit or its time is the way into the editor. */
const CHECKLIST = '- [ ] Call the dentist\n- [ ] Reply to Priya\n- [x] Pick up the parcel';

/** `YYYY-MM-DD` of the device's calendar `days` from `base`. */
function shift(base: string, days: number): string {
  const [y = 0, m = 1, d = 1] = base.split('-').map(Number);
  return new Date(Date.UTC(y, m - 1, d) + days * DAY_MS).toISOString().slice(0, 10);
}

/** The UTC instant of a wall-clock time on `date` in the seeded zone. */
function at(date: string, hhmm: string): number {
  const [y = 0, m = 1, d = 1] = date.split('-').map(Number);
  const [h = 0, min = 0] = hhmm.split(':').map(Number);
  return Date.UTC(y, m - 1, d, h, min) - OFFSET_MIN * 60_000;
}

function words(text: string): number {
  return text.split(/\s+/).filter((w) => /[\p{L}\p{N}]/u.test(w)).length;
}

export function seedJournalEntries(now: Date, newId: () => string): Entry[] {
  const out: Entry[] = [];
  const add = (page: string, body: string, writtenAt: number, editedAt: number | null = null) => {
    out.push({
      id: newId(),
      page_date: page,
      body,
      written_at: writtenAt,
      written_offset_min: OFFSET_MIN,
      written_tz: ZONE,
      edited_at: editedAt,
      word_count: words(body),
      created_at: writtenAt,
      updated_at: editedAt ?? writtenAt,
    });
  };
  const pick = (i: number) => POOL[i % POOL.length] ?? '';

  const pad = (n: number) => String(n).padStart(2, '0');
  const today = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;

  add(today, SHOWCASE, at(today, '06:30'));
  add(today, pick(0), at(today, '07:48'));
  add(today, CHECKLIST, at(today, '09:05'));
  add(today, pick(1), at(today, '13:20'), at(today, '15:05'));

  const yesterday = shift(today, -1);
  add(yesterday, pick(2), at(yesterday, '21:10'));

  // Backfilled: the last entry on this Page was written three days later.
  const lastWeek = shift(today, -6);
  add(lastWeek, pick(3), at(lastWeek, '09:12'));
  add(lastWeek, pick(4), at(lastWeek, '18:40'));
  add(lastWeek, pick(5), at(shift(lastWeek, 3), '09:12'));

  // Last month: guarantees a month divider whatever today is.
  const lastMonth = shift(`${today.slice(0, 7)}-01`, -17);
  add(lastMonth, pick(6), at(lastMonth, '20:05'));

  // Previous year: shows the year in the header, and a written date from another year.
  const lastYear = `${now.getFullYear() - 1}-12-30`;
  add(lastYear, pick(7), at(lastYear, '22:30'));
  add(lastYear, pick(8), at(`${now.getFullYear()}-01-02`, '08:15'));

  // Older Pages for infinite scroll, a few days apart.
  for (let i = 0; i < 60; i++) {
    const date = shift(today, -(30 + i * 3 + (i % 2)));
    const count = 1 + (i % 4);
    for (let j = 0; j < count; j++) {
      add(
        date,
        pick(i * 3 + j + 9),
        at(date, `${pad(7 + ((j * 5 + i) % 15))}:${pad((i * 7 + j * 13) % 60)}`),
      );
    }
  }
  return out;
}
