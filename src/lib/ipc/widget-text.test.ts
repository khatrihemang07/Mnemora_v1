import { describe, expect, it } from 'vitest';
import { completedRepeatMessage, rowDateLabel } from '../../modules/todo/dates';
import { plainTitle } from '../../modules/todo/notes';
import { completedMessage } from '../../modules/todo/tasks';
import { formatHm } from '../ui/when-time';
import fixture from './fixtures/widget-text.json';

// The Todo widget's Rust text rules (crates/todo/src/widget_text.rs) run the same cases.

describe('widget-text fixture, against the app', () => {
  it.each(fixture.plain_title)('plain title: $title', ({ title, plain }) => {
    expect(plainTitle(title)).toBe(plain);
  });

  it.each(fixture.row_date_label)('row date: $date from $today', ({ today, date, label }) => {
    expect(rowDateLabel(date, today)).toBe(label);
  });

  it.each(fixture.format_hm)('time: $hm as $format', ({ hm, format, text }) => {
    expect(formatHm(hm, format as '24h' | '12h')).toBe(text);
  });

  it.each(fixture.completed_message)('completed: $title', ({ title, message }) => {
    expect(completedMessage(title)).toBe(message);
  });

  it.each(fixture.completed_repeat_message)(
    'completed, repeating: $title',
    ({ title, next, today, message }) => {
      expect(completedRepeatMessage(title, next, today)).toBe(message);
    },
  );
});
