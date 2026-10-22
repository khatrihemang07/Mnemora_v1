import { describe, expect, it } from 'vitest';
import { backspaceAt, boxes, CODE_LENGTH, newlyTyped, pasteInto, typeInto } from './code-input';

describe('typing', () => {
  it('advances to the next box after each digit', () => {
    let edit = typeInto('', 0, '4');
    expect(edit).toEqual({ value: '4', focus: 1 });
    edit = typeInto(edit.value, edit.focus, '8');
    expect(edit).toEqual({ value: '48', focus: 2 });
  });

  it('stays on the last box when the code is complete', () => {
    expect(typeInto('4821093', 7, '7')).toEqual({ value: '48210937', focus: 7 });
    expect(typeInto('48210937', 7, '1')).toEqual({ value: '48210931', focus: 7 });
  });

  it('ignores anything that is not a digit', () => {
    expect(typeInto('48', 2, 'a')).toEqual({ value: '48', focus: 2 });
    expect(typeInto('48', 2, ' ')).toEqual({ value: '48', focus: 2 });
  });

  it('replaces a digit in the middle and moves on', () => {
    expect(typeInto('4821', 1, '9')).toEqual({ value: '4921', focus: 2 });
  });

  it('never leaves a gap, even if a far box is typed into', () => {
    expect(typeInto('12', 5, '9')).toEqual({ value: '129', focus: 3 });
  });

  it('fills several boxes when a keyboard sends several digits at once', () => {
    expect(typeInto('', 0, '4821')).toEqual({ value: '4821', focus: 4 });
    expect(typeInto('48', 2, '2109')).toEqual({ value: '482109', focus: 6 });
  });

  it('works out which digit is new when a box already had one', () => {
    expect(newlyTyped('45', '4')).toBe('5');
    expect(newlyTyped('54', '4')).toBe('5');
    expect(newlyTyped('44', '4')).toBe('4');
    expect(newlyTyped('5', '')).toBe('5');
    expect(newlyTyped('4821', '')).toBe('4821');
  });
});

describe('backspace', () => {
  it('clears the digit in the box and stays', () => {
    expect(backspaceAt('4821', 3)).toEqual({ value: '482', focus: 3 });
    expect(backspaceAt('4821', 1)).toEqual({ value: '421', focus: 1 });
  });

  it('goes back and clears the previous digit from an empty box', () => {
    expect(backspaceAt('482', 3)).toEqual({ value: '48', focus: 2 });
  });

  it('does nothing in an empty code', () => {
    expect(backspaceAt('', 0)).toEqual({ value: '', focus: 0 });
  });
});

describe('paste', () => {
  it('fills every box from the first and focuses the last', () => {
    expect(pasteInto('4821 0937')).toEqual({ value: '48210937', focus: 7 });
    expect(pasteInto('4821-0937')).toEqual({ value: '48210937', focus: 7 });
  });

  it('cuts extra digits and keeps a short paste in the first boxes', () => {
    expect(pasteInto('1234567890')).toEqual({ value: '12345678', focus: 7 });
    expect(pasteInto('482')).toEqual({ value: '482', focus: 3 });
  });

  it('ignores a paste with no digits', () => {
    expect(pasteInto('hello')).toBeNull();
    expect(pasteInto('')).toBeNull();
  });
});

describe('another length', () => {
  it('follows the length it is given', () => {
    expect(typeInto('1234', 3, '5', 4)).toEqual({ value: '1235', focus: 3 });
    expect(pasteInto('123456', 4)).toEqual({ value: '1234', focus: 3 });
    expect(boxes('12', 4)).toEqual(['1', '2', '', '']);
  });
});

describe('boxes', () => {
  it('shows one digit per box and blanks after', () => {
    expect(boxes('48')).toEqual(['4', '8', '', '', '', '', '', '']);
    expect(boxes('')).toHaveLength(CODE_LENGTH);
  });
});
