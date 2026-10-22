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
| Shortcuts | ⌘K palette, ⌘N capture (in-app only), ⌘, Settings, ⌘1-9 Modules, ⌘\ sidebar; bare single keys (Q: Todo quick add) only when nothing is being typed or open, see ADR 0011 |

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
- **No flash**: Rust injects the attributes with the window initialization script before first paint (`src-tauri/src/theme_inject.rs`); an inline `<style>` in `index.html` turns them into the page background before `app.css` loads. On load, `applyAppearance` (`src/lib/theme/apply.ts`) keeps the same logic in sync; `system` is re-resolved when the OS theme changes.
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
| `IconButton` | An icon-only action. Needs a `label` (aria-label and tooltip); `bind:ref` anchors a popover to it; `size="xs"` + `variant="quiet"` is the small faint round × for clearing a field or chip |
| `Card` | A hairline-bordered surface that groups content |
| `Dialog` | A small modal needing a decision (confirm, unsaved changes). Optional `children` put choices or a field between the description and the actions (Todo's delete-List Dialog). The actions row wraps on a narrow screen (Todo's "steps still open" Dialog has three buttons) |
| `Sheet` | A bottom sheet: the phone More menu, and the DatePicker and WhenPicker on a phone (`guardEscape` as for `Popover`). Optional `closable` adds a ghost Close button in the title row (on in the DatePicker and calendar sheets). `centered` makes it a panel in the middle of the window on desktop (still a bottom sheet on a phone), `bare` leaves the padding and the title row to the caller (the title is then for screen readers only) and `autoFocus={false}` keeps focus on the sheet itself instead of its first field: together they are Todo's Task sheet. Clicking a toast (Undo) never closes it. On a phone every `Sheet` shows a grip, can be pulled down to close (`sheet-swipe.ts`: the grip, or anywhere while scrolled to the top and not on a field), rides above the on-screen keyboard, keeps clear of the navigation bar (`env(safe-area-inset-bottom)`), and moves toasts to the top of the screen while it is open (`sheet-open.svelte.ts`); `tall` makes it nearly full height (the Task sheet) |
| `CommandPalette` | The ⌘K action list. Mount once in the shell. Filters with `visibleItems` (every typed word in the label); an item with a `queryLabel` is a query action that appears once something is typed ("Search Journal for 'river'") and receives the text in `run` |
| `Tooltip` / `TooltipProvider` | Names or explains a control; 600ms delay, shows the shortcut. Provider mounts once |
| `ResizablePanes` | The desktop sidebar + main split. Wraps paneforge; takes a pixel width and reports dragged pixels |
| `Kbd` | Shows a keyboard shortcut |
| `Toaster` / `toast.ts` | Transient feedback: `notify`, `notifyError`, `notifyUndo` |
| `EmptyState` | Nothing to show yet: icon, sentence, action. Optional `hue` (a Module hue token name, e.g. `--hue-journal`) tints the icon instead of the brand accent |
| `Skeleton` | Loading placeholder (appears after 300ms) |
| `ErrorBanner` | An inline failure with Retry and Copy details |
| `EnvBadge` | The SANDBOX marker (nothing in Production). Also shown next to the phone `PageHeader` title on sandbox builds |
| `PageHeader` | Every page's title. Desktop: sets the top bar title; phone: large title in the content. Optional actions slot, and an optional `icon` with its Module `hue` token name drawn before the title (Todo: the teal icon beside "Inbox"). An optional `heading` snippet replaces icon and title with the page's own (a List's emoji button and click-to-rename title) |
| `SegmentedControl` | Pick one of 2-5 short options. Equal-width segments; a 2x2 grid below 360px for four or more |
| `Switch` | An on/off Setting |
| `Select` | Pick one of many options |
| `Input` | A single-line text field. `bare` drops the frame (border, fill, height, focus ring) for a field inside a frame that shows focus itself, such as Todo's inline "New task" field; `bind:ref` focuses it |
| `AutoTextarea` | A frameless text field that grows with its text instead of scrolling: a Task's title in the sheet, which may wrap but is one line of text (Enter never inserts a line break; the caller handles it through `onkeydown`). `bind:value`, `label`, `bind:ref`. `multiline` lets Shift+Enter add a line break (comments) |
| `TaskCheckbox` | The Task's checkbox: a rounded square turned 45° (a diamond) with an upright tick, ticked in the Todo hue. Hover rings the edge in the Todo hue and shows no tick, so an open Task never looks done. `checked`, `label`, `onchange(next)`; `tone` (`plain`, `p1`-`p3` Priority rings, `overdue` tint) sets the edge and fill. The hit area is larger than the diamond, and the diamond carries the focus ring. Tasks only: Checklist items in notes keep the plain square (`MarkdownView`). `wont` (with `checked`) draws a grey diamond holding a ⊗: a Task marked Won't do, never the done colour |
| `VirtualList` | A long scrolling list (hundreds of rows or an infinite feed). Wraps `@tanstack/svelte-virtual`; rows may vary in height; `onnearend` / `onnearstart` load more below / above; `scrollToIndex(i, smooth?)` / `offsetOf(i)` / `indexAt(offset)` jump to and measure rows (smooth respects reduced motion); rows added or removed above the ones being read never move the view; `isSticky` + `pinned` draw a pinned header overlay (never `position: sticky` inside rows, which misplaces itself in the translated rows) |
| `MarkdownEditor` | Live Markdown writing: CodeMirror 6, loaded on first use. Marks stay visible but faded; shortcuts, list keys, auto-pairs, slash menu, rich-text paste, ⌘/Ctrl-click links. `value` + `onchange`, `onfinish` (Esc, ⌘↵), `bare` for a framed parent, `font`, `spellcheck`, `slashValues`; `focus()` / `run()` via `bind:this`. Never import CodeMirror anywhere else. `checklist` draws `- [ ]` / `- [x]` lines as clickable square boxes (Todo's notes; box colour from `--md-cb-hue`); off, as in the Journal, the marker stays faded text |
| `MarkdownView` | Read-only Markdown. Same parser and typography as the editor (`markdown.css`: `.md*`), symbols hidden, built from Svelte elements, **never `{@html}`**. Links open externally; checklist rows look like checkboxes; with `ontoggle` the whole row ticks on click (hover tint, ringed box; links and text selections excepted) and the box stays keyboard reachable |
| `MarkdownInline` | One line of text with inline Markdown shown: bold, italic, code, links and bare URLs, from the Journal's parser, so block syntax (`#`, `- `, `> `, `1.`) stays as the text it is and strikethrough and highlight are not rendered yet. Built from Svelte elements, never `{@html}`; a link opens the system browser without activating whatever contains it. Todo's Task titles and Comments (`parseInlineMarkdown` in `markdown/inline.ts` is unit-tested). Several lines are fine (comments): breaks are kept, block syntax on any line stays text. `marks` (`{ word, title }[]`, from `mark-words.ts`, unit-tested) underline the last whole-word occurrence of each word with a dotted line and a tooltip: the words capture understood in a Task's title. `lit` (a set of folded words) tints every whole word a search matched, in the Todo hue, for the title of a search result (`litPieces` in `mark-words.ts`, unit-tested) |
| `Menu` | A list of actions beside a button or at a point (right-click, long-press): icon + label + optional shortcut hint, hairline `separatorBefore`, red `danger` item. Controlled (`open`, `anchor` = element or `{x, y}`, `items`, `label`); Bits UI gives arrows, typeahead, Esc and outside-click; dismissing returns focus to the anchor (or `returnFocus`). Entry actions use it; never build ad-hoc popups. An item may carry `iconClass` (a colour for its icon, e.g. the Priority flags), `checked` (a tick at the right edge) and `disabled` (shown but not available yet); shortcut hints are hidden on a phone. `heading` puts a small title above the items for a menu that picks one of several ("Group by", "Sort Lists by") |
| `Popover` | A small floating panel hung off a button (pickers). `bare` removes the padding for a picker that lays itself out edge to edge, `guardEscape` keeps it open when Esc belongs to a panel inside. Controlled (`open`, `anchor` element, `label`); Bits UI gives Esc, outside-click and focus; clicking the anchor toggles it; `focus` picks what to focus, `restoreFocus` where focus goes on close. Marked `data-editor-popup` so an open Entry editor is not closed by clicks inside. `flush` uses the Menu's tight padding for a list of rows; `autofocus={false}` leaves focus alone on open (a hover flyout); `side` may be `right` |
| `EmojiPicker` | Choose one emoji or none: a curated grid of 48 plus "Type or paste any emoji" (picks the moment one appears) and "No emoji". A `Popover` next to `anchor` on desktop, a bottom `Sheet` on a phone; `open` bindable, `value`, `onpick(emoji \| null)`. The grid and the emoji-finding rule are in `emoji.ts` (unit-tested). Todo uses it for a List's emoji |
| `Calendar` | A month grid: dots on `marked` days, a ring on today, `selected`, an optional `range` tint and `disabled` predicate, week start from the `weekStart` prop (core Setting), ‹ › month arrows, a month/year picker on the title and an optional Today button. Keyboard (arrows, PageUp/PageDown, Home/End, Enter) and screen-reader grid roles. `hue` tints today and dots with a Module hue token name. Date maths lives in `calendar.ts` (unit-tested) |
| `DatePicker` | Choose a date: a `Popover` on desktop, a bottom `Sheet` on phone, with Today / Yesterday chips above a `Calendar`. `mode="single"` (`onpick`) or `mode="range"` (two picks, `onrange`; for search); `chips`, `closeOnPick`, `showToday`, `footer` turn it into a browsing calendar (the Journal's go-to-date) |
| `WhenPicker` | Choose a Due date and time compactly (about 420×280): a "Type a date" field on top (a small default reader, pluggable through `parse`, which may answer later: Todo plugs in Rust's parser), quick days with their weekday (Today, Tomorrow, Weekend, Next week, No date), a rolling five-week calendar that starts at this week (‹ › by five weeks, a tiny month label on the 1st, past days disabled, today ringed, task dots that turn amber at 3+) and a slim bar with a Time chip (its panel floats over the calendar; takes `9`, `930p`, `21:30`, `noon`, matches only as you type), a Repeat chip beside it (`WhenRepeatPanel`; `showRepeat` off for "Move all"), a `bar` slot for more chips and a summary at the right. Days a Repeat lands on carry a fainter ring. A `Popover` on desktop, a bottom `Sheet` on a phone; picking a day sets and closes. Controlled: `value`, `onchange`, `counts` + `onrange` for the dots; `showTime` / `showRepeat` / `allowClear` off for "Move all"; `value.repeat` carries the Repeat. The grid model, quick days, time reading, typed-date reader and Repeat rules are pure and unit-tested (`when-grid.ts`, `when-time.ts`, `when-parse.ts`, `when-repeat.ts`, whose rules run the same fixtures as the Rust engine). No native date or time inputs |
| `WhenRepeatPanel` | The Repeat panel inside the `WhenPicker`: it floats over the calendar like the Time panel. Presets in two columns (Every day, Every 2/3/4 days, Every weekday, Weekly on ‹weekday›, Monthly on the ‹date›, Yearly on ‹date›), "Custom…" (every N 1–99 days / weeks / months / years, weekday toggles for weeks), "From the due date" / "From the day it's done" (the latter only with an interval, not chosen weekdays) and a "Next:" line with the next three dates. Controlled: `rule`, `onchange` (null removes). Not used on its own |
| `Chip` | A small pill button that opens a picker (the date chip "Today ▾", the search "Any date ▾"): optional icon, trailing chevron, `tone="warm"` tints it with a Module `hue`. `bind:ref` to anchor a popover. `disabled` dims it (Todo's When and Priority chips on a closed Task) |
| `Pill` | A floating action pill ("↑ Today", "Back to results"): `tone="ink"` neutral dark, or `tone="tinted"` raised with its text in a Module `hue`; optional `icon` and a × (`ondismiss`, `dismissLabel`). The parent positions it with `class` |
| `KeyboardBar` | The phone formatting row above the keyboard while an editor is focused (B, I, H, bullet, checklist, quote, link). Buttons never steal focus; it follows the visual viewport |
| `SettingsLayout` | The section nav + panel frame for Settings (drills in on phone) |
| `SettingSection` | A titled group of rows in a Card |
| `SettingRow` | One Setting: label + hint left, control right (stacked on phone) |
| `SaveBar` | Appears while a form has unsaved edits (Save / Discard) |

## Markdown typography

The editor and the read view share one set of rules in `src/lib/ui/markdown/markdown.css` so that writing and reading line up: body in the display serif by default (`font="sans"` for the UI font), 16px, line-height 1.65; headings 1–3 in the display serif at `--display-weight` (1.35em, 1.18em, 1.05em); blocks 0.55em apart; quotes with a coral bar; highlight from `--warning`; code on `--surface-2`. While editing, every Markdown mark is visible at `--faint`, and the editor reproduces the block gaps (a blank line is 0.55em tall, a block directly followed by another is padded 0.55em), so only the marks themselves move the text.

## Layout

- **Sidebar** (desktop): resizable 200 to 360px (default 248). Dragging below 150px snaps to a 60px icon rail; dragging back past 150px expands. Double-click the divider resets. The toggle button and ⌘\ switch between rail and expanded; it is never fully hidden. Width and rail state persist as Device settings.
- **Nested Module navigation**: a Module may nest its own destinations under its sidebar row (`navTree` in its manifest; today only Todo: Inbox, Today, Upcoming, All, Done, then a "Lists" header with + and the Lists, each with an open count). The tree unfolds when you enter the Module and folds when you leave; while inside, clicking the row or its chevron flips it, and that lasts only until the next enter or leave (`shell/tree-fold.ts`, unit-tested). The unfolded state is a Device setting (`sidebar.todo_open`). The open destination wears the active tint and the Module row is bold. On the icon rail, hovering the Module icon for 600ms or right-clicking it opens a flyout of the same destinations with counts. Right-click (long-press on a phone) a List for Rename, Emoji… and Delete list….
- **Dragging rows** (Todo, desktop mouse only): a whole row is the handle; after 5px a floating label follows the pointer (the Task's title, then what the drop would change in the Todo hue) and a 2px Todo-hue line with a dot shows the place. A sidebar row that takes the Task lights with an inset Todo-hue ring; the dragged row dims to 40%; Esc cancels; the click that ends a drag is swallowed. Views carry `data-*` markers the controller reads (`src/modules/todo/drag.svelte.ts`). Group headings are the uppercase 2xs label with a count (Overdue red, a Priority with its flag, "No date, in a List" with a calendar-off icon), a List heading with its glyph, or a serif day heading; "Group:" and "Sort:" are small ghost buttons opening a `Menu` with a heading.
- **Brand row**: under the traffic-light strip, a 22px `AppMark` and "Mnemora" in the display font; the rail shows only the mark, centred.
- **Top bar** (desktop): a 50px drag region (`data-tauri-drag-region`) with a bottom hairline: sidebar toggle, then the `PageHeader` title, then page actions and a compact "Search... ⌘K" button. Windows and Linux add window controls at the right end.
- **Phone** (under 720px or a mobile OS): no sidebar or top bar. A bottom tab bar shows up to 4 chosen tabs plus **More**. More is highlighted whenever the current Module is not one of the tabs (Settings included) and opens a sheet with the rest. Pages start with a large `PageHeader` title. Phone-only styling uses the `phone:` / `desktop:` variants (driven by `data-phone` on `<html>`, set from `viewport.isPhone`), not a bare width query, so it agrees with the layout switch. A plain width rule (`min-[720px]:`) is only for layout that should follow the window, not the device (the Home grid, Setting rows).
- **Enabled Modules**: a Module is on unless listed in `core.disabled_modules` (so new Modules appear by default). Routes (a disabled Module's URL falls back to Home), sidebar, tab bar and More sheet, ⌘K, ⌘-digit and ⌘N all use only enabled Modules. Switching off hides a Module and keeps its data.
- **Home cards**: Home lists each enabled Module's lazy `homeCard` below the greeting and clock, in nav order, handing each the shell's `ModuleContext` (`ctx`) so its buttons can navigate; with none it shows an `EmptyState`. The Journal's card (H2) is a `Card` with the coral notebook tile and "Journal", a primary Write button at the right, the latest Entry of today as a plain-text quote in the display serif (clamped to 3 lines) and "Today · N entries" below; an empty day says "Nothing written today".
- **Badge**: a manifest's `useBadge` gives the sidebar row and the phone tab a count (`ModuleBadge`): a small pill after the name on the expanded sidebar, over the top right of the icon on the rail and on a tab, `--danger` filled when `alert`, `99+` past 99, nothing at zero. Todo's is due today plus Overdue, red while anything is Overdue. The hook runs while the row renders, so it may create queries.
- **Todo's Home card** follows the Journal's: tile, "Todo", a secondary Add (the floating Quick add, due today) and a ghost Open at the right; up to five Tasks due today as compact rows (diamond with a Priority ring, title, the time at the end in red once passed), then `N overdue · +N more` in small faint text, the overdue part red; with nothing due, "Nothing due today · next: ‹Task›, ‹day›".
- **Palette query actions** may carry a `queryPreview`: a muted line at the end of the row saying what Enter would do with the text (Todo's "Add task" shows the date, time, Priority and List it read). It is asked 120 ms after typing stops and a stale answer never shows.
- **Home (H1)**: greeting card and clock tile in a `1.4fr / 1fr` grid at 720px and wider, equal height; stacked below. The clock has a small uppercase "Local time" label, tight tabular digits and smaller seconds.

## Journal keyboard (R4)

On desktop the feed's Entries are one stop each for the keyboard. ↑ / ↓ move a focus ring (the global `:focus-visible` ring plus the hover tint) between Entries across Pages, scrolling the Entry into view clear of the pinned header and composer fade and loading older Pages at the end. Enter edits; Esc, ⌘/Ctrl+Enter or Done leave the editor with that Entry focused again; ⌫ / Delete deletes with Undo and focuses the next Entry; M opens the move picker; C copies; Esc on an Entry returns to the composer. ↑ from an empty composer focuses the last Entry of Today's Page (the newest Entry before Today when Today has none). The keys apply only while the row itself has focus, never inside an editor, the composer or the search field, and there is no ⌘⌫ (macOS uses it to delete to the line start). The ⋯ menu shows the keys as hints on desktop; Settings ▸ Shortcuts lists them.

## Todo on a phone

Under the large title a segment bar is pinned (`PhoneSegments`, handed to `PageHeader` as `pageHeader.phoneBar`): Today · Upcoming · Inbox and ☰, which opens `ListsSheet` (All, Done, Lists, New list). Swiping the title row (`data-view-swipe`) or the bar moves between the three (`swipedView`); rows own their sideways swipes (`RowTouch`: right completes or reopens, left opens When, a hold lifts the row, then a menu on release or a drag on move). The + button (teal, 56px, bottom right) opens the floating capture sheet for the view's place (`addPlaceFor`). The Todo tab, through the Module manifest's `phoneTab`, goes to Today at the top when tapped inside Todo and opens Quick add when held. Haptics use `navigator.vibrate` (`lib/haptic.ts`; the Android manifest holds VIBRATE) since there is no haptics plugin. `--line-strong` is the outline colour for chips and floating panels that must read on a True black surface; `--line` stays for hairlines.

## Todo widget

The Todo widget is native Android (RemoteViews, Android 12+ only; ADR 0014, 0015, 0016), not Svelte, so it does not use `src/lib/ui`. `docs/design/todo-widget-prototype.html` is the visual reference. It is the one place where tokens are copied by hand.

**Card layout.** A 16 dp rounded card (the launcher's own widget radius on the phones tested). Header: a teal chip with a check, the view name (bold, one line, ellipsized first when space runs out), directly after it the count (11.5 sp, never ellipsized), and in the Today view only, from 240 dp wide (4x2 and up, `SizeMode.LIST_DATE`), the date after a dot ("· Thu 22", the locale's short `EEEd`, 11.5 sp in the count's calm grey, `faint`), then the teal + and the settings gear at the end. The gear exists because some launchers (vivo) have no long-press reconfigure. The list scrolls below. An Undo bar floats over the bottom for about 4 s after a tick; it starts when the bar is drawn, with a grace after the open-steps dialog.

**Header date.** A `TextClock` (`HeaderDate`), so the launcher ticks it over at midnight with none of our code running: it is right even while the list waits for the midnight job. Only Today shows it; Upcoming, Inbox, All and a List do not, and 3x2 drops it. With the date shown the priority is name whole, then count whole, then the date (it ellipsizes or vanishes): the renderer swaps in a weightless name view (`title_fixed`) and the date takes the flexible weight, so a font-size change needs no redraw. Without the date (3x2, or a List name) the name ellipsizes first as before. Its colour follows the Theme like the count, in a day and a night value for Same as app + System. The setup preview shows it; the picker's static preview does not.

**Header count.** Today says "N left", or a red "N overdue" when anything is Overdue; every other view shows its number of open Tasks.

**Size modes** (`SizeMode`, pinned by a unit test, the system picks from a size-responsive RemoteViews): list from 180 x 110 dp (3x2 cells); an "Add a task..." pill from 100 dp wide; a plain teal + below that (1x1). The pill and + add by the Todo widget's view: Today due today, Upcoming tomorrow, a List into that List, Inbox and All into the Inbox.

**Rows.** The diamond (the tick target is 48 dp) with its Priority ring (P1 danger, P2 warning, P3 accent), the title up to two lines then an ellipsis, a List chip in mixed views (Today, Upcoming, Inbox) and the time on the right in muted grey. A row not due today shows its day and time in grey ("Thu 12:00"); an Overdue row shows its due day in danger red. Group headers: a red Overdue, Today, Upcoming days ("Tomorrow · Thu 22"), All by List with the Inbox first. A list is cut at 80 lines; the last line then reads "+N more · Open Mnemora" (teal, group-header style) and opens the view in the app (`RowItems`, pinned by a unit test).

**States.** All wording comes final from Rust. Empty views: "Nothing due today" with the next Task, "Inbox zero", "No open Tasks", "Nothing in the next 7 days". A deleted List: "This List was deleted", tap opens setup (the gear sits in the card's corner). Skeleton rows while loading. "Open Mnemora once to set up your Todo widget" before the app ever ran, "Open Mnemora to update" after an app update with pending migrations, "Couldn't load Tasks · Tap to open Mnemora" on a read error. The Todo widget never creates or migrates the database.

**Themes.** Per Todo widget: Same as app (follows Settings ▸ Theme; System uses the phone's night mode, and then every colour is sent in a day and a night value (`WidgetColors`, API 31 `setColorInt` / `setColorStateList`), so the launcher flips the Todo widget with the phone and no redraw; only the no-background text edge, a layout choice, waits for the next draw), Light, Dark, True black, Material You, Material You dark. Colours are hand-converted from `tokens.css` into `widget_themes.xml` (one set per look):

| Token | Android colour (`widget_<look>_*`) |
|---|---|
| `--surface` (the card; True black: the pure black `--bg`) | `bg` |
| `--surface-2` (Undo bar, field, chips) | `surface2` |
| `--ink`, `--muted`, `--faint` | `ink`, `muted`, `faint` |
| `--danger` (P1, Overdue) | `danger` |
| `--warning` (P2) | `warning` |
| `--accent` (P3) | `accent` |
| `--hue-todo` and its on-colour (chip, +, tick) | `todo`, `on_todo` |

The Priority ring colours keep their meaning in every look. **Material You** reads the system dynamic palette on Android 12+ (`values-v31/widget_themes.xml`: `system_accent2_50` card and `system_accent1_600` for light, `system_accent2_800` and `system_accent1_200` for dark, neutral colours for text); the Priority colours stay fixed. The setup screen itself uses the app's theme.

**Background.** Solid (the theme's surface), See-through (the surface at 72%; Android cannot blur behind a widget) and None. None keeps a faint wash behind the card so text reads on any wallpaper, and uses the `_shadow` layouts (dark looks) or `_glow` layouts (light looks) for the text.

**Sandbox tag.** The Sandbox build labels the picker entry "Todo (Sandbox)" and shows an amber SANDBOX tab at the top centre of the card (`BuildConfig.SANDBOX`). Production never shows it.

**Quick add.** The + opens a native translucent bottom sheet over the home screen in the Todo widget's theme, a copy of Todo's Quick add (placeholder, marked words, chips, # suggestions, List and Priority pickers, a read-only When chip, Notes). The chips and suggestions carry the app's Lucide icons as vector drawables (`qa_*`). Haptic on tick uses the notification usage, because touch-feedback vibration is dropped for background receivers.

## Todo keyboard

On desktop every Task row is a stop for the keyboard, the same idea as the Journal's R4. ↑ / ↓ move a focus ring between Tasks on whatever page is showing (the rows come from the DOM in reading order, so groups, "Completed today" and search results all work): the row gets a tint and a 1.5px accent outline with a soft halo, and its title takes real focus so Tab and screen readers agree (the title's own focus ring is turned off with `data-bare`). The keys are one pure reducer (`task-keymap.ts`, Vitest-tested), fed every key with a description of the scene; it decides, the page runs the action. Bare keys follow ADR 0011 (never in a field, editor or capture box, never with a menu, picker or Dialog open, never with ⌘ ⌃ ⌥ except ⌥↑↓, never a held-down repeat). Every row key (E, D, T, M, 1–4, ⇧Space, ⌥↑↓, Space, Enter, ⌫, Esc) acts on the ringed row only, never on the row under the pointer (ADR 0013). G then T, U, I, A or D goes to a view. In the sheet ⌘↑ ⌘↓ and ⌘↵ are the sheet's own, even inside its editor.

Search lays its results over the page (which stays mounted underneath, inert) the way the Journal's does: a field with a search icon and a clear ×, "N results", then "Open" and "Done" groups of ordinary Task rows (the matched words of the title tinted, the matching note or Comment line under it with a note or speech-bubble icon) and a "Similar matches" group for typos. On a phone a Cancel button closes it, because the page's own header is covered.

## Settings patterns

- **Section** (`SettingSection`): a title above a Card containing rows.
- **Row** (`SettingRow`): label (and hint) on the left, control on the right. Rows stack on phone.
- **Save bar** (N9-B): one draft for the whole page. Edits mark it dirty and the `SaveBar` slides in. Theme, density and text size preview live; Discard reverts. Leaving with unsaved edits asks first.
- **Page-level save bar**: one draft spans every section, so a single Save or Discard covers the whole page.
- **Palette theme commands** (⌘K) apply and save immediately: they are explicit commands, not form edits.
- **Data folder**: changing it from the UI is deferred.
- Phone Settings is a list that drills into one section with a Back button.
- **Module sections**: a Module's `settingsSection` (label, icon, lazy component) is appended after the core sections, on the same draft, Save bar and leave guard, and drills in on phone.
- **Preferences and Device settings together**: a Module section may mix both. Preferences go through the draft and SaveBar; a Device setting (the Journal's Spellcheck) saves at once through its own command, like the sidebar layout.
- **Journal section (Z1)**: Writing font (`SegmentedControl` Serif / Sans), Text width (Narrow / Default / Wide, about 60 / 72 / 90 characters, `35.5rem` / `42.5rem` / `53rem` so it scales with Text size and shrinks to fill narrow windows), Page summaries (`Switch`, on) and Spellcheck (`Switch`, on, this device only). Width is set once as `--journal-width` on the Journal and used by the feed, pinned header, composer and search.
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
