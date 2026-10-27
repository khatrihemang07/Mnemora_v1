/**
 * Whether Enter in an AutoTextarea adds a line break instead of being swallowed. Shift+Enter does in
 * a `multiline` field; `enterBreaks` makes plain Enter do it too (the AI composer on a phone, where
 * the round button sends).
 */
export function enterBreaksLine(opts: {
  shiftKey: boolean;
  multiline: boolean;
  enterBreaks: boolean;
}): boolean {
  return opts.enterBreaks || (opts.multiline && opts.shiftKey);
}
