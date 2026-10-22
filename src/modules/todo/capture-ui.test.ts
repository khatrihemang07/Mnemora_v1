import { describe, expect, it } from 'vitest';
import type { ParseSpan } from '../../lib/ipc/bindings';
import fixture from '../../lib/ipc/fixtures/capture-ui.json';
import {
  applySuggestion,
  backspaceTarget,
  type CaptureWords,
  foldTag,
  type HashQuery,
  hashQuery,
  ignoreKinds,
  ignoreWord,
  listSuggestions,
  markSegments,
  pasteParts,
  preferWord,
  pruneWords,
  recogniseAgain,
  type Suggestion,
  tagOf,
} from './capture';

// The Todo widget's native Quick add repeats these helpers in Kotlin (CaptureRules.kt); its JUnit
// test runs the same fixture.

const words = (w: { ignored: string[]; prefer: object }): CaptureWords => ({
  ignored: w.ignored,
  prefer: w.prefer as Record<string, string>,
});
const spanOf = (s: { kind: string; key: string }) => s as Pick<ParseSpan, 'kind' | 'key'>;

describe('capture-ui fixture, against the TypeScript helpers', () => {
  it.each(fixture.ignore_word)('ignore word: $name', ({ words: w, key, expected }) => {
    expect(ignoreWord(words(w), key)).toEqual(expected);
  });

  it.each(fixture.recognise_again)('recognise again: $name', ({ words: w, key, expected }) => {
    expect(recogniseAgain(words(w), key)).toEqual(expected);
  });

  it.each(fixture.prefer_word)('prefer word: $name', ({ words: w, span, expected }) => {
    expect(preferWord(words(w), span)).toEqual(expected);
  });

  it.each(fixture.ignore_kinds)(
    'ignore kinds: $name',
    ({ words: w, spans, alternatives, kinds, expected }) => {
      const parsed = {
        spans: spans.map(spanOf) as ParseSpan[],
        alternatives: alternatives.map((a) => ({ ...spanOf(a), reason: 'conflict' })) as never,
      };
      expect(ignoreKinds(words(w), parsed, kinds)).toEqual(expected);
    },
  );

  it.each(fixture.prune_words)('prune words: $name', ({ words: w, known, expected }) => {
    expect(
      pruneWords(
        words(w),
        known.map((key) => ({ key }) as ParseSpan),
      ),
    ).toEqual(expected);
  });

  it.each(fixture.backspace_target)(
    'backspace: $name',
    ({ text, selection_start, selection_end, spans, just_unrecognised, expected }) => {
      expect(
        backspaceTarget({
          text,
          selectionStart: selection_start,
          selectionEnd: selection_end,
          spans,
          justUnrecognised: just_unrecognised,
        }),
      ).toBe(expected);
    },
  );

  it.each(fixture.fold_tag)('fold tag: $input', ({ input, expected }) => {
    expect(foldTag(input)).toBe(expected);
  });

  it.each(fixture.tag_of)('tag of: $input', ({ input, expected }) => {
    expect(tagOf(input)).toBe(expected);
  });

  it.each(fixture.hash_query)('hash query: $text at $caret', ({ text, caret, expected }) => {
    expect(hashQuery(text, caret)).toEqual(expected);
  });

  it.each(fixture.list_suggestions)('suggestions: $name', ({ query, lists, expected }) => {
    expect(listSuggestions(query, lists)).toEqual(expected);
  });

  it.each(fixture.apply_suggestion)(
    'apply suggestion: $name',
    ({ text, hash, suggestion, expected }) => {
      expect(applySuggestion(text, hash as HashQuery, suggestion as Suggestion)).toEqual(expected);
    },
  );

  it.each(fixture.mark_segments)('mark segments: $name', ({ text, spans, expected }) => {
    expect(markSegments(text, spans)).toEqual(expected);
  });

  it.each(fixture.paste_parts)('paste parts: $name', ({ pasted, expected }) => {
    expect(pasteParts(pasted)).toEqual(expected);
  });
});
