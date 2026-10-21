/** The curated grid: 48 emoji that name areas of life, eight to a row. */
export const CURATED_EMOJI: readonly string[] = [
  '🏠',
  '💼',
  '🛒',
  '📚',
  '🎯',
  '🧘',
  '✈️',
  '💡',
  '🎨',
  '🔧',
  '💰',
  '🌱',
  '🏃',
  '🎁',
  '🐶',
  '📦',
  '🍳',
  '☕',
  '🎵',
  '🎬',
  '🎮',
  '📷',
  '✏️',
  '💻',
  '📱',
  '🔑',
  '🧹',
  '🧺',
  '🚗',
  '🚲',
  '⚽',
  '🏋️',
  '🩺',
  '💊',
  '🎓',
  '📝',
  '📅',
  '📞',
  '✉️',
  '🧾',
  '🛠️',
  '🌍',
  '⭐',
  '❤️',
  '🔥',
  '🎉',
  '🍎',
  '🌙',
];

const PICTOGRAPH = /\p{Extended_Pictographic}|\p{Regional_Indicator}|⃣/u;

/** The first emoji in `text` (a whole sequence: skin tone, joiners, flags), or null if none. */
export function firstEmoji(text: string): string | null {
  const parts =
    typeof Intl.Segmenter === 'function'
      ? Array.from(
          new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(text),
          (s) => s.segment,
        )
      : [...text];
  return parts.find((g) => PICTOGRAPH.test(g)) ?? null;
}
