/** Pure editing rules for CodeInput: one box per digit, no gaps, `length` digits at most (8 for a Pairing code). */

export const CODE_LENGTH = 8;

/** The code after an edit, and the box that should have focus next. */
export interface CodeEdit {
  value: string;
  focus: number;
}

const onlyDigits = (text: string) => text.replace(/\D/g, '');

/** Digits typed (or autofilled) into box `index`; they land from there on and focus moves past them. */
export function typeInto(
  value: string,
  index: number,
  raw: string,
  length = CODE_LENGTH,
): CodeEdit {
  const digits = onlyDigits(raw);
  // Boxes fill in order, so a digit never lands after a gap.
  const start = Math.min(index, value.length, length - 1);
  if (digits === '') return { value, focus: start };
  const chars = value.split('');
  for (const [offset, digit] of [...digits].entries()) {
    if (start + offset < length) chars[start + offset] = digit;
  }
  return { value: chars.join(''), focus: Math.min(start + digits.length, length - 1) };
}

/** Backspace in box `index`: clears its digit, or, if it is empty, the one before it. */
export function backspaceAt(value: string, index: number): CodeEdit {
  if (index < value.length) {
    return { value: value.slice(0, index) + value.slice(index + 1), focus: index };
  }
  if (index > 0 && value.length > 0) {
    const gone = Math.min(index, value.length) - 1;
    return { value: value.slice(0, gone) + value.slice(gone + 1), focus: gone };
  }
  return { value, focus: Math.max(0, index) };
}

/** Pasted text fills every box from the first; `null` when it holds no digit. */
export function pasteInto(text: string, length = CODE_LENGTH): CodeEdit | null {
  const digits = onlyDigits(text).slice(0, length);
  if (digits === '') return null;
  return { value: digits, focus: Math.min(digits.length, length - 1) };
}

/** What was typed into a box that already held `previous`: the browser appends or prepends it. */
export function newlyTyped(raw: string, previous: string): string {
  const digits = onlyDigits(raw);
  if (previous !== '' && digits.length === 2) {
    if (digits.startsWith(previous)) return digits.slice(1);
    if (digits.endsWith(previous)) return digits.slice(0, 1);
  }
  return digits;
}

/** The digit shown in each box (empty string for a blank one). */
export function boxes(value: string, length = CODE_LENGTH): string[] {
  return Array.from({ length }, (_, i) => value[i] ?? '');
}
