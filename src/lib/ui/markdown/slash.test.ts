import { describe, expect, it } from 'vitest';
import { applyEdit } from './edit';
import { applySlash, canOpenSlash, filterSlash, SLASH_ITEMS, slashQuery } from './slash';

const values = { date: 'Tuesday, October 20', time: '14:05' };
const item = (id: string) => {
  const found = SLASH_ITEMS.find((i) => i.id === id);
  if (!found) throw new Error(id);
  return found;
};
/** Picks `id` for a menu typed as "/query" at `slash`; the caret is right after the query. */
function pick(doc: string, slash: number, id: string) {
  const caret = slash + 1 + (slashQuery(doc, slash, doc.length) ?? '').length;
  return applyEdit(doc, applySlash(doc, slash, caret, item(id), values));
}

describe('canOpenSlash', () => {
  it('opens at the start of the text, a line, or after whitespace', () => {
    expect(canOpenSlash('/', 0)).toBe(true);
    expect(canOpenSlash('a\n/', 2)).toBe(true);
    expect(canOpenSlash('a /', 2)).toBe(true);
    expect(canOpenSlash('a\t/', 2)).toBe(true);
  });

  it('stays closed inside a word or a url', () => {
    expect(canOpenSlash('and/or', 3)).toBe(false);
    expect(canOpenSlash('https://', 6)).toBe(false);
    expect(canOpenSlash('a', 0)).toBe(false);
  });
});

describe('slashQuery', () => {
  it('is the text typed after the slash', () => {
    expect(slashQuery('/', 0, 1)).toBe('');
    expect(slashQuery('a /hea', 2, 6)).toBe('hea');
  });

  it('closes on a space, when the slash is gone, or the caret moves before it', () => {
    expect(slashQuery('/a b', 0, 4)).toBeNull();
    expect(slashQuery('a', 0, 1)).toBeNull();
    expect(slashQuery('x /a', 2, 2)).toBeNull();
  });
});

describe('filterSlash', () => {
  it('lists everything for an empty query, in order', () => {
    expect(filterSlash('').map((i) => i.id)).toEqual([
      'h1',
      'h2',
      'h3',
      'bullet',
      'numbered',
      'task',
      'quote',
      'divider',
      'code',
      'date',
      'time',
    ]);
  });

  it('matches word prefixes of labels and keywords, ignoring case', () => {
    expect(filterSlash('head').map((i) => i.id)).toEqual(['h1', 'h2', 'h3']);
    expect(filterSlash('TODO').map((i) => i.id)).toEqual(['task']);
    expect(filterSlash('d').map((i) => i.id)).toEqual(['divider', 'date']);
    expect(filterSlash('zzz')).toEqual([]);
  });
});

describe('applySlash', () => {
  it('a block at the start of a line takes the line', () => {
    const r = pick('/h2', 0, 'h2');
    expect(r.doc).toBe('## ');
    expect(r.anchor).toBe(3);
  });

  it('a block on a later line leaves earlier lines alone', () => {
    expect(pick('one\n/task', 4, 'task').doc).toBe('one\n- [ ] ');
  });

  it('a block picked mid-line starts on a new line', () => {
    const r = pick('Buy milk /bullet', 9, 'bullet');
    expect(r.doc).toBe('Buy milk\n- ');
    expect(r.anchor).toBe(r.doc.length);
  });

  it('text after the menu moves into the new block', () => {
    const doc = 'Plan /quote the day';
    const edit = applySlash(doc, 5, 11, item('quote'), values);
    expect(applyEdit(doc, edit).doc).toBe('Plan\n> the day');
  });

  it('a marker-only line is replaced, not nested', () => {
    expect(pick('- /h1', 2, 'h1').doc).toBe('# ');
    expect(pick('1. /quote', 3, 'quote').doc).toBe('> ');
  });

  it('divider and code block place the caret sensibly', () => {
    const d = pick('/divider', 0, 'divider');
    expect(d.doc).toBe('---\n');
    expect(d.anchor).toBe(4);
    const c = pick('/code', 0, 'code');
    expect(c.doc).toBe('```\n\n```');
    expect(c.anchor).toBe(4);
  });

  it('date and time insert in the given formats where the slash was', () => {
    expect(pick('Met on /date', 7, 'date').doc).toBe('Met on Tuesday, October 20');
    expect(pick('at /time', 3, 'time').doc).toBe('at 14:05');
  });
});
