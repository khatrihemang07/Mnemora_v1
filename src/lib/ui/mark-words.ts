/**
 * Marking words inside text that is shown in pieces (the text nodes of a rendered title). Each
 * mark names a word and the tooltip it carries; the last whole-word occurrence of it, anywhere in
 * the pieces, is the one marked (the winner of a conflict is usually the last one typed).
 */

export interface WordMark {
  word: string;
  title: string;
}

/** One stretch of a text, with the tooltip when it is a marked word. */
export interface Piece {
  text: string;
  title?: string;
}

const isWordChar = (c: string | undefined): boolean => c !== undefined && /[\p{L}\p{N}_]/u.test(c);

/** Where `word` last occurs in `text` as a whole word (case ignored), or -1. */
function lastWhole(text: string, word: string, before: number): number {
  const low = text.toLowerCase();
  const target = word.toLowerCase();
  for (let at = low.lastIndexOf(target, before); at >= 0; at = low.lastIndexOf(target, at - 1)) {
    const edgeOk = !isWordChar(text[at - 1]) || !isWordChar(target[0]) || target[0] === '#';
    const endOk = !isWordChar(text[at + target.length]) || !isWordChar(target.at(-1));
    if (edgeOk && endOk) return at;
    if (at === 0) break;
  }
  return -1;
}

/**
 * Splits every text into pieces, with the marked words cut out. The result has one list of pieces
 * per input text, in order; joining a list gives its text back.
 */
export function markPieces(texts: string[], marks: WordMark[]): Piece[][] {
  const ranges: { text: number; start: number; end: number; title: string }[] = [];
  for (const mark of marks) {
    if (!mark.word) continue;
    for (let t = texts.length - 1; t >= 0; t--) {
      const text = texts[t] as string;
      const start = lastWhole(text, mark.word, text.length);
      if (start < 0) continue;
      const end = start + mark.word.length;
      const clash = ranges.some((r) => r.text === t && start < r.end && end > r.start);
      if (!clash) ranges.push({ text: t, start, end, title: mark.title });
      break;
    }
  }
  return texts.map((text, t) => {
    const mine = ranges.filter((r) => r.text === t).sort((a, b) => a.start - b.start);
    const out: Piece[] = [];
    let at = 0;
    for (const r of mine) {
      if (r.start > at) out.push({ text: text.slice(at, r.start) });
      out.push({ text: text.slice(r.start, r.end), title: r.title });
      at = r.end;
    }
    if (at < text.length || out.length === 0) out.push({ text: text.slice(at) });
    return out;
  });
}
