import { describe, expect, it } from 'vitest';
import type { Task, TodoCounts } from '../../lib/ipc/bindings';
import type { CaptureFields } from './capture';
import { badgeOf, captureSummary, footerParts, homeCardModel, nothingDueText } from './counts';

const task = (title: string, due_date: string | null = null): Task =>
  ({ id: title, title, due_date }) as Task;

const counts = (over: Partial<TodoCounts> = {}): TodoCounts => ({
  due_today: 0,
  overdue: 0,
  card: [],
  next: null,
  any_tasks: true,
  ...over,
});

describe('badgeOf', () => {
  it('is due today plus Overdue, and red only while something is Overdue', () => {
    expect(badgeOf(counts({ due_today: 3 }))).toEqual({ count: 3, alert: false });
    expect(badgeOf(counts({ due_today: 3, overdue: 2 }))).toEqual({ count: 5, alert: true });
    expect(badgeOf(counts({ overdue: 1 }))).toEqual({ count: 1, alert: true });
  });

  it('shows nothing for a quiet day or before the counts load', () => {
    expect(badgeOf(counts())).toBeNull();
    expect(badgeOf(undefined)).toBeNull();
  });
});

describe('homeCardModel', () => {
  it('shows the Tasks, the Overdue count and how many more are due', () => {
    const card = ['a', 'b', 'c', 'd', 'e'].map((t) => task(t, '2026-10-21'));
    const model = homeCardModel(counts({ due_today: 8, overdue: 2, card }), '2026-10-21');
    expect(model.tasks).toHaveLength(5);
    expect([model.overdue, model.more, model.nothing]).toEqual([2, 3, null]);
  });

  it('has no "more" when everything due fits', () => {
    const card = [task('a', '2026-10-21')];
    expect(homeCardModel(counts({ due_today: 1, card }), '2026-10-21').more).toBe(0);
  });

  it('says nothing is due and hints at the next Task', () => {
    const next = task('Water the **plants**', '2026-10-22');
    expect(homeCardModel(counts({ next }), '2026-10-21').nothing).toBe(
      'Nothing due today · next: Water the plants, Tomorrow',
    );
    expect(nothingDueText(null, '2026-10-21')).toBe('Nothing due today');
  });
});

describe('footerParts', () => {
  it('names each part only when it applies', () => {
    expect(footerParts({ overdue: 2, more: 3 })).toEqual({ overdue: '2 overdue', more: '+3 more' });
    expect(footerParts({ overdue: 0, more: 0 })).toEqual({ overdue: null, more: null });
  });
});

describe('captureSummary', () => {
  const fields = (over: Partial<CaptureFields> = {}): CaptureFields => ({
    listId: null,
    date: null,
    time: null,
    repeat: null,
    priority: 4,
    newList: null,
    fromText: [],
    ...over,
  });
  const summary = (f: CaptureFields, list: string | null = null) =>
    captureSummary(f, list, '2026-10-21', '24h');

  it('describes the date, time, Priority and List it read', () => {
    expect(
      summary(fields({ date: '2026-10-22', time: '18:00', priority: 2, listId: 'x' }), 'Home'),
    ).toBe('Tomorrow · 18:00 · P2 · Home');
  });

  it('names a List that will be created', () => {
    expect(summary(fields({ newList: 'Garden' }))).toBe('New list Garden');
  });

  it('goes to the Inbox when nothing was read', () => {
    expect(summary(fields())).toBe('Inbox');
  });
});
