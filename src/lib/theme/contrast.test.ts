import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

// The contrast of the token pairs that carry text or a control's edge, in each theme. The values
// are read from tokens.css so the test fails when a token changes, not when a copy drifts.

const css = readFileSync(new URL('./tokens.css', import.meta.url), 'utf8').replace(
  /\/\*[\s\S]*?\*\//g,
  '',
);

type Env = Record<string, string>;

interface Rule {
  selectors: string[];
  decls: Env;
}

const rules: Rule[] = [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map(
  ([, selectorText = '', body = '']) => ({
    selectors: selectorText.split(',').map((s) => s.trim()),
    decls: Object.fromEntries(
      [...body.matchAll(/--([a-z0-9-]+)\s*:\s*([^;]*);/g)].map(([, name = '', value = '']) => [
        name,
        value.replace(/\s+/g, ' ').trim(),
      ]),
    ),
  }),
);

type Theme = 'light' | 'dark' | 'oled';

/** The declared tokens for a theme: :root, then the dark block, then the OLED block on top, as the cascade does. */
function tokensFor(theme: Theme): Env {
  const selectors = [':root'];
  if (theme !== 'light') selectors.push(':root[data-theme="dark"]');
  if (theme === 'oled') selectors.push(':root[data-theme="oled"]');
  const env: Env = {};
  for (const selector of selectors) {
    for (const rule of rules) {
      if (rule.selectors.includes(selector)) Object.assign(env, rule.decls);
    }
  }
  return env;
}

function tokenValue(env: Env, name: string): string {
  const value = env[name];
  if (value === undefined) throw new Error(`No token --${name}`);
  return value;
}

type Rgb = [number, number, number];

/** A colour as linear sRGB, from an oklch() value or a var() reference into the same theme. */
function linearRgb(value: string, env: Env): Rgb {
  const ref = value.match(/^var\(--([a-z0-9-]+)\)$/);
  if (ref) return linearRgb(tokenValue(env, ref[1] ?? ''), env);
  const m = value.match(/^oklch\(\s*([\d.]+)(%?)\s+([\d.]+)\s+([\d.]+)\s*\)$/);
  if (!m) throw new Error(`Not an opaque oklch colour: ${value}`);
  const L = m[2] ? Number(m[1]) / 100 : Number(m[1]);
  const C = Number(m[3]);
  const h = (Number(m[4]) * Math.PI) / 180;
  const a = C * Math.cos(h);
  const b = C * Math.sin(h);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const mm = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const clamp = (x: number) => Math.min(1, Math.max(0, x));
  return [
    clamp(4.0767416621 * l - 3.3077115913 * mm + 0.2309699292 * s),
    clamp(-1.2684380046 * l + 2.6097574011 * mm - 0.3413193965 * s),
    clamp(-0.0041960863 * l - 0.7034186147 * mm + 1.707614701 * s),
  ];
}

function luminance([r, g, b]: Rgb): number {
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG contrast ratio between two tokens, in one theme. */
function contrast(theme: Theme, fg: string, bg: string): number {
  const env = tokensFor(theme);
  const a = luminance(linearRgb(tokenValue(env, fg), env)) + 0.05;
  const b = luminance(linearRgb(tokenValue(env, bg), env)) + 0.05;
  return Math.max(a, b) / Math.min(a, b);
}

const THEMES: Theme[] = ['light', 'dark', 'oled'];

describe('token contrast (WCAG)', () => {
  it.each(THEMES)(
    'the Provider status chips read at 4.5:1 or more on their surface (%s)',
    (theme) => {
      expect(contrast(theme, 'warning-ink', 'surface-2')).toBeGreaterThanOrEqual(4.5);
      expect(contrast(theme, 'success', 'surface-2')).toBeGreaterThanOrEqual(4.5);
    },
  );

  it.each(THEMES)(
    'muted text reads at 4.5:1 or more on the popover and sheet surface (%s)',
    (theme) => {
      expect(contrast(theme, 'muted', 'pop')).toBeGreaterThanOrEqual(4.5);
    },
  );

  it.each(['dark', 'oled'] as const)(
    'the Switch off track is at least 3:1 on every surface (%s)',
    (theme) => {
      for (const surface of ['pop', 'surface', 'surface-2', 'bg']) {
        expect(contrast(theme, 'switch-off', surface), `on ${surface}`).toBeGreaterThanOrEqual(3);
      }
    },
  );
});
