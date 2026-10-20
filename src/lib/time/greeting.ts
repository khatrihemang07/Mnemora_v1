export type DayPart = 'morning' | 'afternoon' | 'evening' | 'night';

/** 5-12 morning, 12-17 afternoon, 17-22 evening, otherwise night. */
export function greetingFor(date: Date): DayPart {
  const h = date.getHours();
  if (h >= 5 && h < 12) return 'morning';
  if (h >= 12 && h < 17) return 'afternoon';
  if (h >= 17 && h < 22) return 'evening';
  return 'night';
}

export function greetingText(date: Date, name: string): string {
  const base = `Good ${greetingFor(date)}`;
  const who = name.trim();
  return who ? `${base}, ${who}` : base;
}
