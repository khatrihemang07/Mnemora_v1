import { describe, expect, it } from 'vitest';
import { litPieces, markPieces } from './mark-words';

const join = (pieces: { text: string }[]) => pieces.map((p) => p.text).join('');

describe('markPieces', () => {
  it('cuts a marked word out of its text', () => {
    const [pieces] = markPieces(['Pay rent tomorrow at home'], [{ word: 'tomorrow', title: 'T' }]);
    expect(pieces).toEqual([
      { text: 'Pay rent ' },
      { text: 'tomorrow', title: 'T' },
      { text: ' at home' },
    ]);
  });

  it('gives the text back when joined', () => {
    const texts = ['a tomorrow b', 'tomorrow'];
    for (const [i, pieces] of markPieces(texts, [{ word: 'tomorrow', title: 'T' }]).entries()) {
      expect(join(pieces)).toBe(texts[i]);
    }
  });

  it('marks only the last occurrence, across texts', () => {
    const out = markPieces(['tomorrow or', ' later, tomorrow'], [{ word: 'tomorrow', title: 'T' }]);
    expect(out[0]?.some((p) => p.title)).toBe(false);
    expect(out[1]?.filter((p) => p.title)).toEqual([{ text: 'tomorrow', title: 'T' }]);
  });

  it('matches whole words only, ignoring case', () => {
    expect(markPieces(['fridays'], [{ word: 'fri', title: 'T' }])[0]?.some((p) => p.title)).toBe(
      false,
    );
    expect(markPieces(['See FRI'], [{ word: 'fri', title: 'T' }])[0]?.[1]).toEqual({
      text: 'FRI',
      title: 'T',
    });
  });

  it('marks a phrase and a #List tag', () => {
    const [pieces] = markPieces(
      ['Gym every mon and thu #home'],
      [
        { word: 'every mon and thu', title: 'R' },
        { word: '#home', title: 'L' },
      ],
    );
    expect(pieces?.filter((p) => p.title).map((p) => p.text)).toEqual([
      'every mon and thu',
      '#home',
    ]);
  });

  it('leaves a word that is gone alone, and never marks overlapping words twice', () => {
    expect(markPieces(['nothing here'], [{ word: 'tomorrow', title: 'T' }])).toEqual([
      [{ text: 'nothing here' }],
    ]);
    const [pieces] = markPieces(
      ['next fri'],
      [
        { word: 'next fri', title: 'A' },
        { word: 'fri', title: 'B' },
      ],
    );
    expect(pieces?.filter((p) => p.title)).toEqual([{ text: 'next fri', title: 'A' }]);
  });

  it('copes with empty texts and marks', () => {
    expect(markPieces([''], [{ word: 'x', title: 'T' }])).toEqual([[{ text: '' }]]);
    expect(markPieces(['a'], [{ word: '', title: 'T' }])).toEqual([[{ text: 'a' }]]);
  });
});

describe('litPieces', () => {
  const lit = (words: string[]) => new Set(words);

  it('lights whole matched words, ignoring case and accents', () => {
    expect(litPieces('Buy Café beans', lit(['cafe']))).toEqual([
      { text: 'Buy ', lit: false },
      { text: 'Café', lit: true },
      { text: ' beans', lit: false },
    ]);
  });

  it('lights every occurrence and leaves other words alone', () => {
    const pieces = litPieces('red door, red wall', lit(['red']));
    expect(pieces.filter((p) => p.lit).length).toBe(2);
    expect(pieces.map((p) => p.text).join('')).toBe('red door, red wall');
  });

  it('is one plain piece with nothing to light', () => {
    expect(litPieces('hello', lit([]))).toEqual([{ text: 'hello', lit: false }]);
    expect(litPieces('hello', lit(['world']))).toEqual([{ text: 'hello', lit: false }]);
    expect(litPieces('', lit(['x']))).toEqual([{ text: '', lit: false }]);
  });

  it('does not light part of a longer word', () => {
    expect(litPieces('cafeteria', lit(['cafe']))).toEqual([{ text: 'cafeteria', lit: false }]);
  });
});
