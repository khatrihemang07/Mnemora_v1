import { describe, expect, it } from 'vitest';
import {
  activeSegment,
  addPlaceFor,
  DEFAULT_VIEW,
  listView,
  SMART_VIEWS,
  sameView,
  smartView,
  swipedView,
  viewAfterListGone,
  viewCount,
  viewLabel,
  visibleOn,
} from './views';

describe('views', () => {
  it('lists the five views in sidebar order and opens on Today', () => {
    expect(SMART_VIEWS.map((v) => v.id)).toEqual(['inbox', 'today', 'upcoming', 'all', 'done']);
    expect(DEFAULT_VIEW).toEqual(smartView('today'));
  });

  it('tells views apart by kind and id', () => {
    expect(sameView(smartView('inbox'), smartView('inbox'))).toBe(true);
    expect(sameView(smartView('inbox'), smartView('all'))).toBe(false);
    expect(sameView(listView('a'), listView('a'))).toBe(true);
    expect(sameView(listView('a'), listView('b'))).toBe(false);
    expect(sameView(listView('inbox'), smartView('inbox'))).toBe(false);
  });

  it('names a view', () => {
    expect(viewLabel('upcoming')).toBe('Upcoming');
  });

  it('moves to the Inbox only when the open List is the one that went away', () => {
    expect(viewAfterListGone(listView('a'), 'a')).toEqual(smartView('inbox'));
    expect(viewAfterListGone(listView('a'), 'b')).toEqual(listView('a'));
    expect(viewAfterListGone(smartView('all'), 'a')).toEqual(smartView('all'));
  });
});

describe('viewCount', () => {
  const sidebar = { inbox_count: 4, today_count: 3, overdue_count: 1 };

  it('counts the Inbox plainly and Today in red while anything is Overdue', () => {
    expect(viewCount(sidebar, 'inbox')).toEqual({ count: 4, alert: false });
    expect(viewCount(sidebar, 'today')).toEqual({ count: 3, alert: true });
    expect(viewCount({ ...sidebar, overdue_count: 0 }, 'today')).toEqual({
      count: 3,
      alert: false,
    });
  });

  it('has no count for the other views or before the sidebar loads', () => {
    expect(viewCount(sidebar, 'upcoming')).toEqual({ count: 0, alert: false });
    expect(viewCount(undefined, 'today')).toEqual({ count: 0, alert: false });
  });
});

describe('phone views', () => {
  it('lights the segment of the view, or ☰ for the rest', () => {
    expect(activeSegment(smartView('upcoming'))).toBe('upcoming');
    expect(activeSegment(smartView('all'))).toBe('more');
    expect(activeSegment(smartView('done'))).toBe('more');
    expect(activeSegment(listView('a'))).toBe('more');
  });

  it('swipes between the three segments and stops at the ends', () => {
    expect(swipedView(smartView('today'), 1)).toEqual(smartView('upcoming'));
    expect(swipedView(smartView('upcoming'), 1)).toEqual(smartView('inbox'));
    expect(swipedView(smartView('inbox'), 1)).toBeNull();
    expect(swipedView(smartView('today'), -1)).toBeNull();
    expect(swipedView(smartView('inbox'), -1)).toEqual(smartView('upcoming'));
  });

  it('does not swipe away from All, Done or a List', () => {
    expect(swipedView(smartView('all'), 1)).toBeNull();
    expect(swipedView(listView('a'), -1)).toBeNull();
  });

  it('adds into the place the view stands for', () => {
    expect(addPlaceFor(smartView('today'), '2026-10-21', '2026-10-22')).toEqual({
      date: '2026-10-21',
      listId: null,
    });
    expect(addPlaceFor(smartView('upcoming'), '2026-10-21', '2026-10-22')).toEqual({
      date: '2026-10-22',
      listId: null,
    });
    expect(addPlaceFor(listView('a'), '2026-10-21', '2026-10-22')).toEqual({
      date: null,
      listId: 'a',
    });
    expect(addPlaceFor(smartView('inbox'), '2026-10-21', '2026-10-22')).toEqual({
      date: null,
      listId: null,
    });
  });
});

describe('visibleOn', () => {
  const today = '2026-10-21';
  const t = (due_date: string | null, list_id: string | null = null) => ({ due_date, list_id });
  it('puts due-or-earlier in Today and later in Upcoming', () => {
    expect(visibleOn(smartView('today'), t('2026-10-21'), today)).toBe(true);
    expect(visibleOn(smartView('today'), t('2026-10-20'), today)).toBe(true);
    expect(visibleOn(smartView('today'), t('2026-10-22'), today)).toBe(false);
    expect(visibleOn(smartView('today'), t(null), today)).toBe(false);
    expect(visibleOn(smartView('upcoming'), t('2026-10-22'), today)).toBe(true);
    expect(visibleOn(smartView('upcoming'), t('2026-10-21'), today)).toBe(false);
  });
  it('shows the Inbox its own and undated ones from Lists', () => {
    expect(visibleOn(smartView('inbox'), t('2026-10-25'), today)).toBe(true);
    expect(visibleOn(smartView('inbox'), t(null, 'a'), today)).toBe(true);
    expect(visibleOn(smartView('inbox'), t('2026-10-25', 'a'), today)).toBe(false);
  });
  it('shows a List its Tasks, All everything open and Done nothing', () => {
    expect(visibleOn(listView('a'), t(null, 'a'), today)).toBe(true);
    expect(visibleOn(listView('a'), t(null, 'b'), today)).toBe(false);
    expect(visibleOn(smartView('all'), t('2026-12-01', 'b'), today)).toBe(true);
    expect(visibleOn(smartView('done'), t(null), today)).toBe(false);
  });
});
