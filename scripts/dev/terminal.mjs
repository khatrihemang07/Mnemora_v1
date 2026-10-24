// The terminal: printed lines, and raw keypresses when stdin is a terminal.

export function log(message) {
  process.stdout.write(`${message}\n`);
}

/** Sends each keypress to `onKey`. Raw mode delivers Ctrl-C as a character, not a signal. */
export function attachKeys(onKey) {
  process.stdin.setRawMode(true);
  process.stdin.setEncoding('utf8');
  process.stdin.on('data', (chunk) => {
    for (const key of chunk) onKey(key);
  });
  process.stdin.resume();
}

export function restoreTerminal() {
  if (process.stdin.isTTY && process.stdin.isRaw) {
    process.stdin.setRawMode(false);
  }
}
