import { describe, expect, it } from 'vitest';
import type { GroupKey, TaskGroup, TodoList } from '../../lib/ipc/bindings';
import {
  addDefaults,
  adoptSummary,
  canDrag,
  DONE_IN_LISTS_OPTIONS,
  GROUP_DEFAULTS,
  GROUP_OPTIONS,
  GROUP_PREF_KEYS,
  groupButtonLabel,
  groupHeading,
  groupTakesTasks,
  LIST_SORT_OPTIONS,
  optionLabel,
  sortMenuLabel,
  TASK_SORT_OPTIONS,
} from './arrange';

const TODAY = '2026-10-21';
const home = { id: 'L1', name: 'Home', emoji: '🏠' } as TodoList;
const lists = new Map([[home.id, home]]);
const values = (view: keyof typeof GROUP_OPTIONS) => GROUP_OPTIONS[view].map((o) => o.value);

describe('the option tables', () => {
  it('offer each view exactly the groupings it supports, the default among them', () => {
    expect(values('today')).toEqual(['none', 'priority', 'list']);
    expect(values('upcoming')).toEqual(['day', 'priority', 'list']);
    expect(values('inbox')).toEqual(['none', 'priority', 'due']);
    expect(values('all')).toEqual(['list', 'priority', 'due', 'none']);
    expect(values('list')).toEqual(['none', 'priority', 'due']);
    for (const view of Object.keys(GROUP_OPTIONS) as (keyof typeof GROUP_OPTIONS)[]) {
      expect(values(view)).toContain(GROUP_DEFAULTS[view]);
    }
    expect(GROUP_DEFAULTS).toEqual({
      today: 'priority',
      upcoming: 'day',
      inbox: 'none',
      all: 'list',
      list: 'none',
    });
  });

  it('name the Preferences the smart views keep their grouping in', () => {
    expect(GROUP_PREF_KEYS).toEqual({
      today: 'todo.group_today',
      upcoming: 'todo.group_upcoming',
      inbox: 'todo.group_inbox',
      all: 'todo.group_all',
    });
  });

  it('list the Task sorts, the List sorts and the Done choices', () => {
    expect(TASK_SORT_OPTIONS.map((o) => o.value)).toEqual([
      'manual',
      'due',
      'priority',
      'title',
      'added',
    ]);
    expect(LIST_SORT_OPTIONS.map((o) => o.label)).toEqual([
      'Manual',
      'Name A–Z',
      'Name Z–A',
      'Most open',
      'Recently created',
    ]);
    expect(DONE_IN_LISTS_OPTIONS.map((o) => o.value)).toEqual([
      'today_shown',
      'folded',
      'shown',
      'hidden',
    ]);
  });

  it('label the Group button and let only the manual order be dragged', () => {
    expect(groupButtonLabel('today', 'priority')).toBe('Group: Priority');
    expect(groupButtonLabel('inbox', 'due')).toBe('Group: Due date');
    expect(optionLabel(TASK_SORT_OPTIONS, 'added')).toBe('Recently added');
    expect(sortMenuLabel(TASK_SORT_OPTIONS[0] as never)).toBe('Manual (drag)');
    expect(sortMenuLabel(TASK_SORT_OPTIONS[1] as never)).toBe('Due date');
    expect(canDrag('manual')).toBe(true);
    expect(canDrag('title')).toBe(false);
  });
});

describe('group headings', () => {
  const at = (key: GroupKey, showToday = false) =>
    groupHeading(key, { today: TODAY, lists, showToday });

  it('say what each group is', () => {
    expect(at({ kind: 'plain' })).toEqual({ kind: 'none' });
    expect(at({ kind: 'overdue' })).toEqual({ kind: 'label', label: 'Overdue', tone: 'danger' });
    expect(at({ kind: 'priority', priority: 1 })).toMatchObject({ label: 'P1', priority: 1 });
    expect(at({ kind: 'priority', priority: 4 })).toMatchObject({ label: 'No priority' });
    expect(at({ kind: 'list', list_id: 'L1' })).toEqual({ kind: 'list', list: home });
    expect(at({ kind: 'list', list_id: null })).toEqual({ kind: 'list', list: null });
    expect(at({ kind: 'due', bucket: 'overdue' })).toMatchObject({
      label: 'Overdue',
      tone: 'danger',
    });
    expect(at({ kind: 'due', bucket: 'week' })).toMatchObject({ label: 'Next 7 days' });
    expect(at({ kind: 'no_date_in_list' })).toMatchObject({
      label: 'No date, in a List',
      icon: 'calendar-off',
    });
  });

  it("name today's own group only under Overdue, and later days with their date", () => {
    const today: GroupKey = { kind: 'day', date: TODAY };
    expect(at(today)).toEqual({ kind: 'none' });
    expect(at(today, true)).toMatchObject({ label: 'Today' });
    expect(at({ kind: 'day', date: '2026-10-22' })).toEqual({
      kind: 'day',
      name: 'Tomorrow',
      small: '22 Oct',
    });
  });
});

describe('adding and dropping in a group', () => {
  it('start a new Task from the group: its day and List, never its Priority', () => {
    expect(addDefaults([{ kind: 'day', date: '2026-10-24' }])).toEqual({ date: '2026-10-24' });
    expect(
      addDefaults([
        { kind: 'day', date: TODAY },
        { kind: 'priority', priority: 1 },
      ]),
    ).toEqual({
      date: TODAY,
    });
    expect(addDefaults([{ kind: 'list', list_id: 'L1' }])).toEqual({ date: null, listId: 'L1' });
    expect(addDefaults([{ kind: 'list', list_id: null }])).toEqual({ date: null, listId: null });
    expect(addDefaults([])).toEqual({ date: null });
  });

  it('take no new Tasks into Overdue or the Inbox extra group', () => {
    const group = (key: GroupKey) => ({ key, adopt: [], tasks: [] }) as TaskGroup;
    expect(groupTakesTasks(group({ kind: 'overdue' }))).toBe(false);
    expect(groupTakesTasks(group({ kind: 'due', bucket: 'overdue' }))).toBe(false);
    expect(groupTakesTasks(group({ kind: 'no_date_in_list' }))).toBe(false);
    expect(groupTakesTasks(group({ kind: 'due', bucket: 'today' }))).toBe(true);
    expect(groupTakesTasks(group({ kind: 'plain' }))).toBe(true);
  });

  it('summarise what a drop changes, and nothing when the Task already matches', () => {
    const task = { due_date: TODAY, priority: 4, list_id: null };
    const ctx = { today: TODAY, lists };
    expect(adoptSummary([{ kind: 'day', date: '2026-10-24' }], task, ctx)).toBe('Sat 24 Oct');
    expect(adoptSummary([{ kind: 'priority', priority: 1 }], task, ctx)).toBe('Priority 1');
    expect(adoptSummary([{ kind: 'list', list_id: 'L1' }], task, ctx)).toBe('🏠 Home');
    expect(
      adoptSummary(
        [
          { kind: 'day', date: '2026-10-22' },
          { kind: 'priority', priority: 2 },
        ],
        task,
        ctx,
      ),
    ).toBe('Thu 22 Oct · Priority 2');
    expect(
      adoptSummary(
        [
          { kind: 'day', date: TODAY },
          { kind: 'list', list_id: null },
        ],
        task,
        ctx,
      ),
    ).toBeNull();
  });
});
