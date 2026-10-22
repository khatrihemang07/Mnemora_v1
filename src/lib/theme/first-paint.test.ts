import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

// index.html paints the background before app.css loads, so it repeats --bg and color-scheme.
// Read from disk: Vitest turns CSS imports (even `?raw`) into empty strings.
const read = (path: string) => readFileSync(new URL(path, import.meta.url), 'utf8');
const indexHtml = read('../../../index.html');
const tokensCss = read('./tokens.css');
// The last match wins, as in CSS: oled also appears in the shared dark+oled rule before its own.
const block = (css: string, selector: string) =>
  [...css.matchAll(new RegExp(`(?:^|\\n)${selector}\\s*\\{([^}]*)\\}`, 'g'))].at(-1)?.[1] ?? '';
const prop = (body: string, name: string) =>
  body.match(new RegExp(`(?:^|[\\s;])${name}:\\s*([^;]+);`))?.[1];

const DARK_AND_OLED = ':root\\[data-theme="dark"\\],\\s*:root\\[data-theme="oled"\\]';

const THEMES = [
  { theme: 'light', tokens: ':root', html: 'html' },
  { theme: 'dark', tokens: DARK_AND_OLED, html: 'html\\[data-theme="dark"\\]' },
  { theme: 'oled', tokens: ':root\\[data-theme="oled"\\]', html: 'html\\[data-theme="oled"\\]' },
];

describe('first-paint style in index.html', () => {
  it.each(THEMES)('$theme matches --bg in tokens.css', ({ tokens, html }) => {
    const bg = prop(block(tokensCss, tokens), '--bg');
    expect(bg).toBeDefined();
    expect(prop(block(indexHtml, `\\s*${html}`), 'background')).toBe(bg);
  });

  it('sets color-scheme like tokens.css', () => {
    expect(prop(block(indexHtml, '\\s*html'), 'color-scheme')).toBe('light');
    expect(prop(block(indexHtml, '\\s*html\\[data-theme="dark"\\]'), 'color-scheme')).toBe('dark');
    expect(prop(block(indexHtml, '\\s*html\\[data-theme="oled"\\]'), 'color-scheme')).toBe('dark');
    expect(prop(block(tokensCss, DARK_AND_OLED), 'color-scheme')).toBe('dark');
  });
});
