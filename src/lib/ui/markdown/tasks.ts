/** A checklist item line: optional quote marks and indent, a bullet or number, then `[ ]`/`[x]`. */
const TASK_LINE = /^([ \t>]*(?:[-*+]|\d+[.)])[ \t]+\[)([ xX])(\](?:[ \t\r]|$))/;

/**
 * Flips the checkbox on the 0-based source `line`, or returns null when it is not a task. Mirrors
 * `toggle_task` in crates/journal, which is the rule; the UI only uses this to show a tick at once
 * while the command runs, and the browser mock uses it as its stand-in.
 */
export function toggleTaskLine(body: string, line: number): string | null {
  const lines = body.split('\n');
  const text = lines[line];
  const m = text === undefined ? null : TASK_LINE.exec(text);
  if (text === undefined || !m) return null;
  const head = m[1] ?? '';
  lines[line] = `${head}${m[2] === ' ' ? 'x' : ' '}${text.slice(head.length + 1)}`;
  return lines.join('\n');
}
