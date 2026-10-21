import { describe, expect, it } from 'vitest';
import {
  DEFAULT_VIEW,
  listView,
  SMART_VIEWS,
  sameView,
  smartView,
  viewAfterListGone,
  viewCount,
  viewLabel,
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
