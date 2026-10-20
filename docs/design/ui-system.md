# UI system

The recorded UI decisions, so the whole app stays consistent. Visual reference: [ui-pitch.html](ui-pitch.html) (open in a browser). Journal decisions: [journal-prototype.html](journal-prototype.html) (chosen options frozen in its `LOCKED` map).

## Decisions

| Area | Pick |
|---|---|
| Vibrance | **S1** calm canvas, vibrant moments (gradient hero from Module hues, accents) |
| Direction | **D5** Warm Studio: warm neutrals, radii 8/12/16, hairline structure |
| Accents | **C1** Jewel: brand violet, Home amber, Journal coral, Todo teal, AI violet, Settings slate |
| Fonts | **F2** Inter (UI), Fraunces (display), JetBrains Mono |
| Icons | **I3** Lucide via `unplugin-icons` (`~icons/lucide/<name>`) |
| Home | **H1** greeting card + clock tile, side by side on desktop, stacked on phone |
| App icon | **A4** Memory Spiral, warm amber to coral (`src-tauri/icons/source/app-icon.svg`) |
| Navigation | Desktop sidebar; phone 4 user-chosen tabs + More, no centre "+" |
| Themes | Light, Dark, System, True black (OLED) |
| Density | Comfortable or Compact; the phone always uses touch sizes |
| Shortcuts | ⌘K palette, ⌘N capture (in-app only), ⌘, Settings, ⌘1-9 Modules, ⌘\ sidebar |

| Detail | Pick | Value |
|---|---|---|
| N1 Focus | B | Soft glow halo: 1.5px accent outline + 4px `--ring`, keyboard focus only |
| N2 Hover | A | Neutral tint (`--hover`) |
| N3 Press | A | `scale(0.96)` |
| N4 Cards | A | 1px hairline border (`--line`) |
| N5 Toasts | A | Bottom-right stack, max 3, 4s, pause on hover; above the tab bar on phone |
| N6 Empty states | A | Icon + one sentence + action |
| N7 Loading | B | Pulse skeleton, shown only after 300ms |
| N8 Errors | A | Inline banner with Retry and Copy details |
| N9 Saving | B | Save bar appears when changed. Settings and Module forms. Long-form writing autosaves. Theme and density preview live; Discard reverts |
| N10 Deleting | A | Delete at once, Undo toast |
| N11 Scrollbars | A | Thin, appear on hover |
| N12 Tooltips | B | **600ms** delay |
| N13 Motion | A | Snappy: 140ms ease-out |
| N14 Page transitions | B | Fade, 120ms |
| N15 Timestamps | A | Relative under 7 days, absolute on hover |
| N16 Shortcut hints | A | In tooltips, menus and the palette |
| N17 Rail | A | Icons + tooltip, 600ms delay |
| N18 Settings layout | A | Section nav + panel: Appearance, General, Navigation, Data & storage, Modules, Shortcuts, About |

## Token architecture

Three layers, all in `src/lib/theme/tokens.css` and `src/app.css`:

1. **Primitive**: raw OKLCH values and scales, written once per theme inside `:root` / `[data-theme]`. Only `tokens.css` contains colour literals.
2. **Semantic**: names that say what a value is *for*. Components use only these.
   - Surfaces and ink: `--bg`, `--side`, `--surface`, `--surface-2`, `--pop`, `--ink`, `--muted`, `--faint`, `--line`, `--hover`, `--active`
   - Brand and feedback: `--accent`, `--on-accent`, `--accent-soft`, `--ring`, `--danger`, `--danger-soft`, `--on-danger`, `--success`, `--warning`, `--on-warning`
   - Module hues (C1): `--hue-home`, `--hue-journal`, `--hue-todo`, `--hue-ai`, `--hue-settings`
   - Hero (S1): `--hero-from`, `--hero-via`, `--hero-to`, `--hero-ink`, `--hero-ink-muted` (darkened so white text passes AA)
   - Shape and depth: `--r1`/`--r2`/`--r3` (8/12/16), `--card-border`, `--elev-card`, `--elev-pop`, `--scroll-thumb`, `--scrim`
   - Type: `--f-ui`, `--f-display`, `--f-mono`, `--display-weight`, `--display-track`. Sizes use Tailwind's scale (`text-xs` 12px, `text-sm` 14px, `text-base`, `text-lg`) plus one added step, `text-2xs` (11px, tab labels and badges). No arbitrary `text-[...]` sizes
   - Layout and density: `--row`, `--pad`, `--gap`, `--fs`, `--titlebar-h`
   - Motion: `--dur`, `--dur-exit`, `--dur-fade`, `--ease`
3. **Component**: Tailwind utilities generated from the semantic layer in `app.css` (`bg-surface`, `text-muted`, `border-line`, `rounded-md`, `shadow-card`, `font-display`, `ease-out`, `text-hero-ink`). Tailwind's default palette and scales are removed, so a raw colour utility does not exist.

Add a new value by adding a semantic token (with light, dark and OLED values), then mapping it in `app.css`. Never use a literal in a Component.

## Themes

- `data-theme` on `<html>` is `light`, `dark` or `oled`; `data-theme-pref` keeps the choice (including `system`). `data-density`, `data-text-size` and `data-reduce-motion` carry the other appearance Settings, and `data-env` carries the Environment.
- **No flash**: Rust injects the attributes with the window initialization script before first paint (`src-tauri/src/theme_inject.rs`). On load, `applyAppearance` (`src/lib/theme/apply.ts`) keeps the same logic in sync; `system` is re-resolved when the OS theme changes.
- Dark uses lighter surfaces for elevation, not shadows. True black sets `--bg` and `--side` to pure black and barely lifts surfaces.

## Density and text size

- Comfortable: `--row` 34px, `--pad` 16px, `--gap` 14px. Compact: 28px, 12px, 10px (smaller `--fs`).
- On phones (under 720px) density is forced to touch sizes: `--row` 44px, `--pad` 16px, `--gap` 14px.
- Text size S/M/L scales the root font-size (90% / 100% / 112.5%), so everything in `rem` follows.

## Motion

| Token | Value | Use |
|---|---|---|
| `--dur` | 140ms | Hover, press, toggles, entrances |
| `--dur-exit` | 98ms | Exits (30% shorter) |
| `--dur-fade` | 120ms | Page fade |
| `--ease` | `cubic-bezier(0.2, 0.7, 0.2, 1)` | The one easing (Tailwind `ease-out`) |

All three durations become 0ms under the Reduce motion Setting or the OS preference. Animate `transform` and `opacity`.

**Page transitions**: navigation through `router.navigate` runs inside `document.startViewTransition` when available and motion is allowed (`::view-transition-old/new(root)`: 120ms fade in, exit 30% shorter). Otherwise the `.route-fade` keyframe fade plays; `data-view-transition` on `<html>` suppresses it during a View Transition so it never runs twice. Plain link clicks use the keyframe fade.

## Component inventory (`src/lib/ui`)

| Component | Use it when |
|---|---|
| `AppMark` | You need the app's brand mark (sidebar). Brand art, the one place with literal colours |
| `Button` | Any text action: primary, secondary, ghost or danger; sizes sm/md/lg |
| `IconButton` | An icon-only action. Needs a `label` (aria-label and tooltip) |
| `Card` | A hairline-bordered surface that groups content |
| `Dialog` | A small modal needing a decision (confirm, unsaved changes) |
| `Sheet` | The phone bottom sheet (the More menu) |
| `CommandPalette` | The ⌘K action list. Mount once in the shell |
| `Tooltip` / `TooltipProvider` | Names or explains a control; 600ms delay, shows the shortcut. Provider mounts once |
| `ResizablePanes` | The desktop sidebar + main split. Wraps paneforge; takes a pixel width and reports dragged pixels |
| `Kbd` | Shows a keyboard shortcut |
| `Toaster` / `toast.ts` | Transient feedback: `notify`, `notifyError`, `notifyUndo` |
| `EmptyState` | Nothing to show yet: icon, sentence, action. Optional `hue` (a Module hue token name, e.g. `--hue-journal`) tints the icon instead of the brand accent |
| `Skeleton` | Loading placeholder (appears after 300ms) |
| `ErrorBanner` | An inline failure with Retry and Copy details |
| `EnvBadge` | The SANDBOX marker (nothing in Production) |
| `PageHeader` | Every page's title. Desktop: sets the top bar title; phone: large title in the content. Optional actions slot |
| `SegmentedControl` | Pick one of 2-5 short options. Equal-width segments; a 2x2 grid below 360px for four or more |
| `Switch` | An on/off Setting |
| `Select` | Pick one of many options |
| `Input` | A single-line text field |
| `VirtualList` | A long scrolling list (hundreds of rows or an infinite feed). Wraps `@tanstack/svelte-virtual`; rows may vary in height; `onnearend` loads more; `scrollToTop()` respects reduced motion; `isSticky` + `pinned` draw a pinned header overlay (never `position: sticky` inside rows, which misplaces itself in the translated rows) |
| `Textarea` | A multi-line text field that grows with its content up to a cap. Unstyled box: the parent supplies the frame |
| `MarkdownEditor` | Live Markdown writing: CodeMirror 6, loaded on first use. Marks stay visible but faded; shortcuts, list keys, auto-pairs, slash menu, rich-text paste, ⌘/Ctrl-click links. `value` + `onchange`, `onfinish` (Esc, ⌘↵), `bare` for a framed parent, `font`, `spellcheck`, `slashValues`; `focus()` / `run()` via `bind:this`. Never import CodeMirror anywhere else |
| `MarkdownView` | Read-only Markdown. Same parser and typography as the editor (`markdown.css`: `.md*`), symbols hidden, built from Svelte elements, **never `{@html}`**. Links open externally; checklist rows look like checkboxes and take an `ontoggle` hook |
| `KeyboardBar` | The phone formatting row above the keyboard while an editor is focused (B, I, H, bullet, checklist, quote, link). Buttons never steal focus; it follows the visual viewport |
| `SettingsLayout` | The section nav + panel frame for Settings (drills in on phone) |
| `SettingSection` | A titled group of rows in a Card |
| `SettingRow` | One Setting: label + hint left, control right (stacked on phone) |
| `SaveBar` | Appears while a form has unsaved edits (Save / Discard) |

## Markdown typography

The editor and the read view share one set of rules in `src/lib/ui/markdown/markdown.css` so that writing and reading line up: body in the display serif by default (`font="sans"` for the UI font), 16px, line-height 1.65; headings 1–3 in the display serif at `--display-weight` (1.35em, 1.18em, 1.05em); blocks 0.55em apart; quotes with a coral bar; highlight from `--warning`; code on `--surface-2`. While editing, every Markdown mark is visible at `--faint`, and the editor reproduces the block gaps (a blank line is 0.55em tall, a block directly followed by another is padded 0.55em), so only the marks themselves move the text.

## Layout

- **Sidebar** (desktop): resizable 200 to 360px (default 248). Dragging below 150px snaps to a 60px icon rail; dragging back past 150px expands. Double-click the divider resets. The toggle button and ⌘\ switch between rail and expanded; it is never fully hidden. Width and rail state persist as Device settings.
- **Brand row**: under the traffic-light strip, a 22px `AppMark` and "Mnemora" in the display font; the rail shows only the mark, centred.
- **Top bar** (desktop): a 50px drag region (`data-tauri-drag-region`) with a bottom hairline: sidebar toggle, then the `PageHeader` title, then page actions and a compact "Search... ⌘K" button. Windows and Linux add window controls at the right end.
- **Phone** (under 720px or a mobile OS): no sidebar or top bar. A bottom tab bar shows up to 4 chosen tabs plus **More**. More is highlighted whenever the current Module is not one of the tabs (Settings included) and opens a sheet with the rest. Pages start with a large `PageHeader` title.
- **Enabled Modules**: a Module is on unless listed in `core.disabled_modules` (so new Modules appear by default). Routes (a disabled Module's URL falls back to Home), sidebar, tab bar and More sheet, ⌘K, ⌘-digit and ⌘N all use only enabled Modules. Switching off hides a Module and keeps its data.
- **Home cards**: Home lists each enabled Module's lazy `homeCard` below the greeting and clock, in nav order; with none it shows an `EmptyState`.
- **Home (H1)**: greeting card and clock tile in a `1.4fr / 1fr` grid at 720px and wider, equal height; stacked below. The clock has a small uppercase "Local time" label, tight tabular digits and smaller seconds.

## Settings patterns

- **Section** (`SettingSection`): a title above a Card containing rows.
- **Row** (`SettingRow`): label (and hint) on the left, control on the right. Rows stack on phone.
- **Save bar** (N9-B): one draft for the whole page. Edits mark it dirty and the `SaveBar` slides in. Theme, density and text size preview live; Discard reverts. Leaving with unsaved edits asks first.
- **Page-level save bar**: one draft spans every section, so a single Save or Discard covers the whole page.
- **Palette theme commands** (⌘K) apply and save immediately: they are explicit commands, not form edits.
- **Data folder**: changing it from the UI is deferred.
- Phone Settings is a list that drills into one section with a Back button.
- **Module sections**: a Module's `settingsSection` (label, icon, lazy component) is appended after the core sections, on the same draft, Save bar and leave guard, and drills in on phone.
- **Modules** section: one `Switch` per Module, bound to `core.disabled_modules` through the draft. Home and Settings are `required` and show a disabled, checked switch.

## Do

- Use tokens and `src/lib/ui` Components for everything.
- Give every icon-only control a label, and show shortcuts in tooltips.
- Handle all four states: loading (skeleton), empty, error, content.
- Use Lucide icons through `~icons/lucide/*`.
- Check light, dark and True black, comfortable and compact, and a 360px-wide phone.

## Don't

- Don't use raw colours (hex, rgb, named) or Tailwind palette classes. The `AppMark` is the only exception; `scripts/check-colors.sh` (part of `make lint`) enforces it.
- Don't import `bits-ui`, `svelte-sonner` or `paneforge` outside `src/lib/ui` (`ResizablePanes` wraps paneforge).
- Don't add one-off controls inside a Module; extend or add a Component instead.
- Don't hardcode durations or easings; use `--dur*` and `--ease`.
- Don't use `setInterval` or poll; see the [standards](../engineering/standards.md).
- Don't use pure shadows for elevation in dark themes.
