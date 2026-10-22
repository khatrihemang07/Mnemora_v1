import { describe, expect, it } from 'vitest';
import {
  completedRepeatMessage,
  landedIn,
  rowDateLabel,
  whenChipLabel,
} from '../../modules/todo/dates';
import { plainTitle } from '../../modules/todo/notes';
import { completedMessage, quickAddedMessage } from '../../modules/todo/tasks';
import { meaningText, type UnderstoodKind } from '../../modules/todo/understood';
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

  it.each(fixture.landed_in)('landed in: $due_date / $list', ({ due_date, list, today, text }) => {
    expect(landedIn({ due_date }, list, today)).toBe(text);
  });

  it.each(fixture.quick_added_message)('added toast: $title', ({ title, where, message }) => {
    expect(quickAddedMessage(title, where)).toBe(message);
  });

  it.each(fixture.meaning_text)(
    'chip meaning: $kind $value',
    ({ kind, value, today, format, text }) => {
      expect(
        meaningText(kind as UnderstoodKind, value, {
          today,
          timeFormat: format as '24h' | '12h',
          listName: () => null,
        }),
      ).toBe(text);
    },
  );

  it.each(fixture.when_chip_label)(
    'When chip: $label',
    ({ date, time, repeat, today, format, label }) => {
      expect(
        whenChipLabel(
          {
            due_date: date,
            due_time: time,
            repeat_freq: repeat?.freq ?? null,
            repeat_every: repeat?.every ?? null,
            repeat_days: repeat?.days ?? null,
            repeat_mode: repeat?.mode ?? null,
          },
          today,
          format as '24h' | '12h',
        ),
      ).toBe(label);
    },
  );
});
