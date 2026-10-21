import { describe, expect, it } from 'vitest';
import type { Parsed, ParseSpan } from '../../lib/ipc/bindings';
import {
  applySuggestion,
  backspaceTarget,
  type CaptureBase,
  foldTag,
  hashQuery,
  ignoreKinds,
  ignoreWord,
  listSuggestions,
  markSegments,
  NO_WORDS,
  pasteParts,
  preferWord,
  pruneWords,
  recogniseAgain,
  resolveFields,
  suggestionKey,
  tagOf,
  unchangedRanges,
} from './capture';

const span = (over: Partial<ParseSpan>): ParseSpan => ({
  key: 'date:tomorrow:1',
  kind: 'date',
  start: 9,
  end: 17,
  word: 'tomorrow',
  value: '2026-10-22',
  new_list: false,
  ...over,
});

const parsed = (over: Partial<Parsed> = {}): Parsed => ({
  spans: [],
  alternatives: [],
  ignored: [],
  date: null,
  time: null,
  repeat: null,
  priority: null,
  list_id: null,
  new_list: null,
  ...over,
});

describe('words kept as text', () => {
  it('keeps a word as text, once', () => {
    const w = ignoreWord(NO_WORDS, 'date:tomorrow:1');
    expect(w.ignored).toEqual(['date:tomorrow:1']);
    expect(ignoreWord(w, 'date:tomorrow:1')).toBe(w);
  });

  it('reads a word again', () => {
    const w = recogniseAgain(ignoreWord(NO_WORDS, 'a'), 'a');
    expect(w.ignored).toEqual([]);
  });

  it('forgets a "use instead" pick for a word that is kept as text', () => {
    const picked = preferWord(NO_WORDS, span({}));
    expect(picked.prefer).toEqual({ date: 'date:tomorrow:1' });
    expect(ignoreWord(picked, 'date:tomorrow:1').prefer).toEqual({});
  });

  it('a "use instead" pick replaces the earlier pick of its kind', () => {
    const w = preferWord(preferWord(NO_WORDS, span({ key: 'a' })), span({ key: 'b' }));
    expect(w.prefer).toEqual({ date: 'b' });
  });

  it('a chip picked by hand turns the words of its kinds into text', () => {
    const p = parsed({
      spans: [span({ key: 'date:today:1' }), span({ key: 'prio:p1:1', kind: 'prio' })],
      alternatives: [{ ...span({ key: 'date:tomorrow:1' }), reason: 'conflict' }],
    });
    const w = ignoreKinds({ ignored: [], prefer: { date: 'date:tomorrow:1', prio: 'x' } }, p, [
      'date',
    ]);
    expect(w.ignored.sort()).toEqual(['date:today:1', 'date:tomorrow:1']);
    expect(w.prefer).toEqual({ prio: 'x' });
    expect(ignoreKinds(NO_WORDS, null, ['date'])).toEqual(NO_WORDS);
  });

  it('forgets decisions about words that are no longer in the text', () => {
    const w = { ignored: ['a', 'gone'], prefer: { date: 'a', prio: 'gone' } };
    expect(pruneWords(w, [span({ key: 'a' })])).toEqual({ ignored: ['a'], prefer: { date: 'a' } });
    const same = { ignored: ['a'], prefer: {} };
    expect(pruneWords(same, [span({ key: 'a' })])).toBe(same);
  });
});

describe('⌫ right after a recognised word', () => {
  const base = {
    text: 'Pay rent tomorrow',
    selectionStart: 17,
    selectionEnd: 17,
    spans: [{ key: 'date:tomorrow:1', end: 17 }],
    justUnrecognised: false,
  };

  it('un-recognises the word the caret follows', () => {
    expect(backspaceTarget(base)).toBe('date:tomorrow:1');
  });

  it('also when one space sits between the word and the caret', () => {
    expect(
      backspaceTarget({
        ...base,
        text: 'Pay rent tomorrow ',
        selectionStart: 18,
        selectionEnd: 18,
      }),
    ).toBe('date:tomorrow:1');
    expect(
      backspaceTarget({
        ...base,
        text: 'Pay rent tomorrow  ',
        selectionStart: 19,
        selectionEnd: 19,
      }),
    ).toBeNull();
  });

  it('deletes as usual the second time, mid-word, or with a selection', () => {
    expect(backspaceTarget({ ...base, justUnrecognised: true })).toBeNull();
    expect(backspaceTarget({ ...base, selectionStart: 14, selectionEnd: 14 })).toBeNull();
    expect(backspaceTarget({ ...base, selectionStart: 9 })).toBeNull();
    expect(backspaceTarget({ ...base, spans: [] })).toBeNull();
  });
});

describe('#List suggestions', () => {
  const lists = [
    { id: '1', name: 'Homework', emoji: null },
    { id: '2', name: 'Home', emoji: '🏠' },
    { id: '3', name: 'Work', emoji: null },
    { id: '4', name: 'Side projects', emoji: null },
    { id: '5', name: 'Café', emoji: null },
  ];

  it('finds the tag being typed at the caret', () => {
    expect(hashQuery('Fix tap #ho', 11)).toEqual({ start: 8, query: 'ho' });
    expect(hashQuery('#ho', 3)).toEqual({ start: 0, query: 'ho' });
    expect(hashQuery('Fix tap #', 9)).toEqual({ start: 8, query: '' });
    expect(hashQuery('Fix #ho tomorrow', 7)).toEqual({ start: 4, query: 'ho' });
  });

  it('finds nothing away from a tag', () => {
    expect(hashQuery('Fix tap', 7)).toBeNull();
    expect(hashQuery('Fix tap #ho ', 12)).toBeNull();
    expect(hashQuery('Fix a#ho', 8)).toBeNull();
    expect(hashQuery('Fix #home', 6)).toBeNull();
  });

  it('suggests Lists by folded prefix, an exact name first', () => {
    expect(listSuggestions('home', lists).map((s) => s.kind === 'list' && s.id)).toEqual([
      '2',
      '1',
    ]);
    expect(listSuggestions('WO', lists).map((s) => s.kind === 'list' && s.id)).toEqual(['3']);
    expect(listSuggestions('cafe', lists).map((s) => s.kind === 'list' && s.id)).toEqual(['5']);
    expect(listSuggestions('sideproj', lists).map((s) => s.kind === 'list' && s.id)).toEqual(['4']);
    expect(listSuggestions('', lists)).toHaveLength(5);
  });

  it('offers a new List for a name nothing starts with, but not for numbers', () => {
    expect(listSuggestions('Garage', lists)).toEqual([{ kind: 'new', name: 'Garage' }]);
    expect(listSuggestions('123', lists)).toEqual([]);
    expect(listSuggestions('', [])).toEqual([]);
  });

  it('spells a List name as a tag', () => {
    expect(tagOf('Side projects')).toBe('Sideprojects');
    expect(tagOf('R&D')).toBe('RD');
    expect(foldTag('Łódź')).toBe('lodz');
  });

  it('moves with the arrows, round the ends', () => {
    expect(suggestionKey('ArrowDown', false, { count: 3, index: 0 })).toEqual({
      type: 'move',
      index: 1,
    });
    expect(suggestionKey('ArrowDown', false, { count: 3, index: 2 })).toEqual({
      type: 'move',
      index: 0,
    });
    expect(suggestionKey('ArrowUp', false, { count: 3, index: 0 })).toEqual({
      type: 'move',
      index: 2,
    });
  });

  it('accepts with Enter or Tab, so Enter never adds the Task', () => {
    expect(suggestionKey('Enter', false, { count: 1, index: 0 })).toEqual({ type: 'accept' });
    expect(suggestionKey('Tab', false, { count: 2, index: 1 })).toEqual({ type: 'accept' });
  });

  it('leaves Shift+Enter, other keys and an empty list alone, and Esc dismisses', () => {
    expect(suggestionKey('Enter', true, { count: 1, index: 0 })).toBeNull();
    expect(suggestionKey('a', false, { count: 1, index: 0 })).toBeNull();
    expect(suggestionKey('Enter', false, { count: 0, index: 0 })).toBeNull();
    expect(suggestionKey('Escape', false, { count: 1, index: 0 })).toEqual({ type: 'dismiss' });
  });

  it('completes the tag and puts the caret after the space', () => {
    const hash = { start: 8, query: 'ho' };
    const home = { kind: 'list' as const, id: '2', name: 'Home', emoji: null };
    expect(applySuggestion('Fix tap #ho', hash, home)).toEqual({
      text: 'Fix tap #Home ',
      caret: 14,
    });
    expect(applySuggestion('Fix tap #ho tomorrow', hash, home)).toEqual({
      text: 'Fix tap #Home tomorrow',
      caret: 14,
    });
    const side = { kind: 'list' as const, id: '4', name: 'Side projects', emoji: null };
    expect(applySuggestion('x #si', { start: 2, query: 'si' }, side).text).toBe('x #Sideprojects ');
    const fresh = { kind: 'new' as const, name: 'Garage' };
    expect(applySuggestion('x #Garage', { start: 2, query: 'Garage' }, fresh).text).toBe(
      'x #Garage ',
    );
  });
});

describe('accepting a #List suggestion never duplicates the tag', () => {
  const text = 'Call mum tomorrow to tell today at 6pm #ho';
  const lists = [{ id: '1', name: 'Home', emoji: null }];
  const hash = hashQuery(text, text.length);
  const pick = listSuggestions(hash?.query ?? '', lists)[0];

  // Tab, Enter and a click all go through the same completion, so one case covers each path.
  it('Tab and Enter accept the highlighted row; the tag is replaced, not appended', () => {
    for (const key of ['Tab', 'Enter']) {
      expect(suggestionKey(key, false, { count: 1, index: 0 })).toEqual({ type: 'accept' });
      if (!hash || !pick) throw new Error('no suggestion');
      expect(applySuggestion(text, hash, pick).text).toBe(
        'Call mum tomorrow to tell today at 6pm #Home ',
      );
    }
  });

  it('a click accepts the same row with the same result', () => {
    if (!hash || !pick) throw new Error('no suggestion');
    const done = applySuggestion(text, hash, pick);
    expect(done.text.match(/#Home/g)).toHaveLength(1);
    expect(done.caret).toBe(done.text.length);
    // Completing again finds no tag at the caret, so nothing more is inserted.
    expect(hashQuery(done.text, done.caret)).toBeNull();
  });
});

describe('Quick edit leaves the old title alone', () => {
  it('freezes everything when nothing changed', () => {
    expect(unchangedRanges('Pay rent tomorrow', 'Pay rent tomorrow')).toEqual([
      { start: 0, end: 17 },
    ]);
  });

  it('freezes the old words and not the new ones', () => {
    expect(unchangedRanges('Pay rent tomorrow', 'Pay rent tomorrow fri')).toEqual([
      { start: 0, end: 17 },
    ]);
    expect(unchangedRanges('Pay rent tomorrow', 'Pay rent tomorrow fri')).not.toContainEqual({
      start: 18,
      end: 21,
    });
  });

  it('freezes words on both sides of an edit in the middle', () => {
    expect(unchangedRanges('Buy milk tomorrow', 'Buy oat milk tomorrow')).toEqual([
      { start: 0, end: 3 },
      { start: 8, end: 21 },
    ]);
  });

  it('freezes only whole words', () => {
    // "tomorrow" became "tomorrows": that is a changed word.
    expect(unchangedRanges('Pay tomorrow', 'Pay tomorrows')).toEqual([{ start: 0, end: 3 }]);
    expect(unchangedRanges('Pay tomorrow', 'Pay Xtomorrow')).toEqual([{ start: 0, end: 3 }]);
  });

  it('freezes a word that moved to the front', () => {
    expect(unchangedRanges('tomorrow', 'x tomorrow')).toEqual([{ start: 2, end: 10 }]);
  });

  it('freezes nothing when everything changed or the old title was empty', () => {
    expect(unchangedRanges('abc', 'xyz')).toEqual([]);
    expect(unchangedRanges('', 'tomorrow')).toEqual([]);
  });

  it('freezes what is left after a word is deleted', () => {
    expect(unchangedRanges('Pay rent tomorrow', 'Pay rent')).toEqual([{ start: 0, end: 8 }]);
  });
});

describe('pasting', () => {
  it('leaves a single line to the field', () => {
    expect(pasteParts('Buy milk')).toBeNull();
    expect(pasteParts('Buy milk\n')).toBeNull();
    expect(pasteParts('')).toBeNull();
  });

  it('makes one Task of several lines: the first is the title, the rest the notes', () => {
    expect(pasteParts('Buy milk\nand eggs\r\n- [ ] butter')).toEqual({
      title: 'Buy milk',
      notes: 'and eggs\n- [ ] butter',
    });
  });

  it('skips leading blank lines and trims', () => {
    expect(pasteParts('\n\n  Buy milk  \n\n  more  \n')).toEqual({
      title: 'Buy milk',
      notes: 'more',
    });
  });

  it('lets a title over the limit spill into the notes', () => {
    const got = pasteParts(`${'a'.repeat(510)}\nnext`);
    expect(got?.title).toHaveLength(500);
    expect(got?.notes).toBe(`${'a'.repeat(10)}\nnext`);
  });

  it('has no Task in blank lines', () => {
    expect(pasteParts('\n \n')).toBeNull();
  });
});

describe('the fields the Task gets', () => {
  const base: CaptureBase = {
    listId: 'l-work',
    date: '2026-10-25',
    time: null,
    repeat: null,
    priority: 4,
  };

  it('uses where the box opened when the text says nothing', () => {
    expect(resolveFields(parsed(), {}, base)).toMatchObject({
      date: '2026-10-25',
      listId: 'l-work',
      priority: 4,
    });
    expect(resolveFields(null, {}, base).date).toBe('2026-10-25');
  });

  it('lets the words win over a chip, and a chip over where the box opened', () => {
    const words = parsed({
      spans: [span({})],
      date: '2026-10-22',
      priority: 1,
    });
    const got = resolveFields(words, { date: '2026-10-30', priority: 3, listId: null }, base);
    expect(got).toMatchObject({ date: '2026-10-22', priority: 1, listId: null });
    expect(resolveFields(parsed(), { date: '2026-10-30', priority: 3 }, base)).toMatchObject({
      date: '2026-10-30',
      priority: 3,
    });
  });

  it('keeps an Upcoming day for a time with no date', () => {
    const time = parsed({
      spans: [span({ kind: 'time', key: 't' })],
      time: '09:00',
      date: '2026-10-22',
    });
    expect(resolveFields(time, {}, base)).toMatchObject({ date: '2026-10-25', time: '09:00' });
  });

  it("takes the parser's today-or-tomorrow for a time when the box opened with no day", () => {
    const time = parsed({
      spans: [span({ kind: 'time', key: 't' })],
      time: '09:00',
      date: '2026-10-22',
    });
    expect(resolveFields(time, {}, { ...base, date: null }).date).toBe('2026-10-22');
  });

  it('takes a List from the words, or offers a new one', () => {
    expect(resolveFields(parsed({ list_id: 'l-home' }), {}, base).listId).toBe('l-home');
    const fresh = resolveFields(parsed({ new_list: 'Garage' }), {}, base);
    expect(fresh).toMatchObject({ listId: null, newList: 'Garage' });
  });

  it('lets a chip clear what the box opened with', () => {
    expect(resolveFields(parsed(), { date: null }, base).date).toBeNull();
    expect(resolveFields(parsed(), { listId: null }, base).listId).toBeNull();
  });

  it('says which kinds the words set', () => {
    const got = resolveFields(
      parsed({ spans: [span({}), span({ kind: 'prio', key: 'p' })] }),
      {},
      base,
    );
    expect(got.fromText).toEqual(['date', 'prio']);
  });
});

describe('the highlight behind the field', () => {
  it('cuts the text at the recognised words', () => {
    expect(
      markSegments('Pay rent tomorrow p1', [
        { start: 9, end: 17 },
        { start: 18, end: 20 },
      ]),
    ).toEqual([
      { text: 'Pay rent ', mark: false },
      { text: 'tomorrow', mark: true },
      { text: ' ', mark: false },
      { text: 'p1', mark: true },
    ]);
  });

  it('gives the text back when joined, and copes with nothing recognised', () => {
    expect(markSegments('Buy milk', [])).toEqual([{ text: 'Buy milk', mark: false }]);
    expect(markSegments('', [])).toEqual([]);
    const segs = markSegments('tonight', [
      { start: 0, end: 7 },
      { start: 0, end: 7 },
    ]);
    expect(segs).toEqual([{ text: 'tonight', mark: true }]);
  });

  it('ignores a span that no longer fits the text', () => {
    expect(markSegments('hi', [{ start: 0, end: 9 }])).toEqual([{ text: 'hi', mark: false }]);
  });
});
