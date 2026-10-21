# Coding standards

These are binding for every change, by people and by agents.

## Principles

- **Modular**: small units with one job and a narrow interface.
- **Scalable**: adding a Module touches its own folder plus two registration lines.
- **Maintainable**: boring, readable code and named constants over clever code.
- **Fast and efficient**: no polling, no needless re-renders, small bundles, lazy-load what is not on the first screen.

## Where code goes

| Code | Location |
|---|---|
| Data, rules, validation, persistence | Rust: `crates/core` (shared) or `crates/<module>` (one Module) |
| Tauri wiring: commands, window, theme injection | `src-tauri/src` (thin; logic belongs in core) |
| Shared UI building blocks | `src/lib/ui` |
| Router wrapper, IPC, theme, time, Settings store | `src/lib/*` |
| App chrome (sidebar, tab bar, registry) | `src/shell` |
| A Module's screens and queries | `src/modules/<id>` |

Rules:

- One concern per file. If a file needs "and" to describe it, split it.
- Rust owns data and rules. The UI is a thin typed view: it renders, collects input and calls commands. It never re-implements validation or business rules.
- Modules never import each other. They share only core contracts (`Module`, `EventBus`, Settings) and typed events.
- All UI comes from `src/lib/ui`. Modules do not import `bits-ui`, `svelte-sonner` or `paneforge` directly; add a Component instead.
- Colours, radii, shadows and durations come from tokens (see [ui-system.md](../design/ui-system.md)). No raw colours: `scripts/check-colors.sh` (run by `make lint`) fails on hex, `rgb(`, `hsl(` or `oklch(` in `src/` outside `tokens.css`, `AppMark.svelte` and tests.
- IPC types come from the generated `src/lib/ipc/bindings.ts`. Never edit it by hand; run `make bindings`.

## Naming

Use the [glossary](../../GLOSSARY.md): **Module**, **Component**, **Settings**, **Production**, **Sandbox**, **Environment**, **Preference**, **Device setting**, **Home**. Do not call a Module a "feature" or "plugin", and do not call a Component a "widget".

- Rust: `snake_case` functions and modules, `PascalCase` types, `SCREAMING_CASE` constants. Settings keys are `module.key`.
- Svelte: `PascalCase.svelte` for Components, `kebab-case.ts` for logic, `*.svelte.ts` for rune state, `*.test.ts` next to the source.
- Commands are `noun_verb` in Rust (`settings_save`) and camelCase in the bindings (`settingsSave`).

## Error handling

- Rust returns `AppResult<T>`. `AppError { code, message, details }` is the only error that crosses IPC. `message` is user-presentable; `details` is technical and only shown behind "Copy details".
- Each crate maps its own errors with `thiserror`, then into `AppError` via `From`. Choose the narrowest `ErrorCode` (`validation`, `unknown_key`, `not_found`, `io`, `database`, `internal`).
- `unwrap` and `expect` are denied by clippy outside tests. Propagate with `?`, or handle the failure deliberately.
- The UI turns a failed command into an inline `ErrorBanner` (with retry) or a toast, using `unwrap()` from `src/lib/ipc/unwrap.ts`.

## Logging

Uses `tracing`; the level is a Device setting.

| Level | Use for |
|---|---|
| `error` | Something failed and the user is affected |
| `warn` | Recovered or degraded (a backup failed, a file was skipped) |
| `info` | Lifecycle: startup, migrations applied, backups made |
| `debug` | Detail useful when diagnosing |
| `trace` | Very chatty; off by default |

Never log user content (journal text, names, tokens). Log ids and counts. UI errors are forwarded to Rust (`log_ui_error`) by the global handlers, so they land in the same file. Logs roll daily per Environment and are pruned after 7 days.

## Comments

Short, and about **why**, not what. Document every public Rust item (`missing_docs` is on). Do not narrate code. When a rule is non-obvious (a workaround, an ordering requirement, a platform quirk), say so in one line next to it.

## Testing

- Test-first at the seams: settings validation, migrations, backups, paths, and pure UI logic (formatting, layout maths, tab rules).
- Rust: unit tests beside the code in `#[cfg(test)]` for pure functions; database behaviour is tested against an in-memory SQLite (`db::open_in_memory`) or a `tempfile` dir. Business rules are tested through the commands with `TestApp`, not inside the crate ([ADR 0009](../adr/0009-rule-tests-at-the-command-layer.md)).
- UI: Vitest, with `*.test.ts` next to the source. Keep logic in plain `.ts` so it is testable without a DOM.
- The browser mock (`src/lib/ipc/mock-*.ts`) re-implements Rust rules, so each rule it mirrors has a case in a shared JSON fixture (`src/lib/ipc/fixtures/`) that both a `TestApp` test and a Vitest test against the mock run.
- No E2E for now ([ADR 0006](../adr/0006-no-e2e-tests.md)). Use the manual smoke checklist instead (below).
- `make test` and `make lint` must pass before you finish.

## Manual smoke checklist

`make dev`, then:

1. Theme, density and sidebar (expanded and rail) look right; Settings save/discard works; values persist after a restart.
2. Home shows the greeting and clock, plus an empty state while no Module provides a card; Module cards appear for enabled Modules only.
3. Settings ▸ Modules: Home and Settings are checked and disabled. Switching a Module off and saving removes it from the sidebar, phone tabs and More, ⌘K and ⌘-digit; its URL falls back to Home. Switching it on and saving restores all of them. Unsaved switches show the leave prompt.
4. A Module's Settings section appears after the core sections (and drills in on phone) only while the Module is on.
5. ⌘N runs the first enabled Module's create action; with none it shows "Nothing to create yet". With the Journal on it opens the Journal on Today with the composer focused, from any screen.

### Journal

1. ⌘2 opens the Journal; it also appears in the sidebar (and can be switched off and on in Settings ▸ Modules). An empty Journal shows "A blank page".
2. Desktop: the composer ("Write…") is focused on open; on a phone it is not. It is as wide as the text column.
3. Type one character: exactly one Entry is created (check again after typing more; no second Entry). Pause 600ms and keep typing: the same Entry updates. A blank or whitespace-only composer never creates anything. Type one character, clear it and finish (⌘↵, Esc, ✓, click away or leave the Journal): the Entry is discarded with "Entry discarded" and Undo, which brings it back; while focus stays in the box, empty text just waits.
4. Finish with ⌘↵ / Ctrl+↵, Esc and the ✓ button (one each): the composer clears and the Entry appears on Today's Page with its time and line breaks kept.
5. Quit and reopen the app: the Entries are still there, newest Page first, oldest Entry first within a Page.
6. Leaving the Journal mid-sentence still saves the text. Closing the window within about 600ms of the last keystroke can lose that last pause's text (accepted risk).
7. No Entry text appears in the log files (Settings ▸ Data & storage ▸ open the log folder).
8. Feed: each Page header shows the written date in the serif ("Tuesday, 20 October") with a coral Today/Yesterday badge, the year only for other years, and (desktop only) "N entries · N words" with singular/plural. The header stays pinned while you scroll its Page. A faint uppercase month divider appears where the month changes. Dates with no Entries are skipped.
9. Entries: the time sits in a left margin on desktop and above the text on a phone; a hairline separates Entries; text is serif with blank-line paragraph gaps. An Entry written on another day than its Page shows a short date (stacked above the time on desktop, on the time line on a phone), with the year if it differs. Hovering the time (desktop only) shows "Written Monday, 19 October 2026 at 09:12 (Asia/Kolkata)". An edited Entry shows a faint "edited" (desktop hover shows the last edit time). The written-timestamp and edit-time tooltips are desktop-only; a phone has none. Switching 12h/24h in Settings changes the times.
10. Scroll a long Journal (with many Pages): older Pages load before you reach the end with no button; scrolling stays smooth. After about a screen below Today, "↑ Today" appears; it scrolls back to Today smoothly (instantly under Reduce motion) and disappears there.
11. An empty Today with older Pages shows "Nothing yet today. The box below is ready."; a brand-new Journal shows the coral notebook icon, "A blank page" and "Start typing in the box below". A slow load shows a skeleton only after 300ms; a failed load shows "The Journal couldn't load." with Retry and Copy details.

12. Markdown reading: an Entry renders without symbols: headings (serif, heavier), bold, italic, strikethrough, highlight, quote with a bar, divider, bullet/numbered/checklist (checkbox look), inline code, fenced code, links and bare URLs (click opens the browser). A table stays as plain text. Open `/journal?seed=journal` in the dev browser to see every feature in the first Entry.
13. Inline editing: click an Entry (not a link) to edit it in place: it lifts onto a card, the rest dims to about 35%, and the text does not shift apart from the symbols. Esc, ⌘↵, a click outside and Done each finish; ⌘Z / ⌘⇧Z undo only inside the open editor. Edits within 10 minutes of writing do not show "edited"; later ones do, and each one moves the time forward. Clear the text and press Done: the toast "Empty entries aren't saved. Kept the previous text." shows and the old text stays. Quit mid-edit and reopen: the text is there.
14. Markdown symbols: while editing, `**`, `#`, `>`, `-`, backticks and link addresses stay visible but faded.
15. Shortcuts (⌘ is Ctrl off macOS): ⌘B bold, ⌘I italic, ⌘⇧X strikethrough, ⌘⇧H highlight, ⌘E code, ⌘K link with a selection (with none, the palette opens), ⌘⇧8 / 7 / 9 bullet / numbered / checklist, ⌘⌥1–3 headings.
16. Slash menu: `/` at a line start or after a space lists H1–3, bullet, numbered, checklist, quote, divider, code block, today's date and the current time (in the General Settings formats). Arrow keys, Enter, Tab and click pick; Esc or a space closes it and leaves the `/` ("and/or" is plain text). A block item picked mid-line starts on a new line.
17. List keys and pairs: Enter continues a list and ends it on an empty item; Tab / ⇧Tab nest and lift; Backspace right after a marker removes it. Typing `(`, `[`, a backtick or `**` closes the pair; a selection is wrapped; typing the closer steps over it; Backspace between an empty pair removes both. Quotes and underscores are never closed. No smart quotes or dashes.
18. Paste: rich text from a web page keeps bold, italic, links, lists, headings and quotes as Markdown; ⌘⇧V pastes plain text. ⌘/Ctrl-click a link while editing opens it; a plain click places the cursor.
19. Save failure (dev browser): open `/journal?seed=journal&fail=journal_update`, edit an Entry and pause: the banner "Couldn't save this entry. Your text is safe here." appears with Retry and Copy details and the text stays. In the console run `__journalMock.failUpdate = false`, then Retry or type on: it saves and the banner goes. Leaving the Journal while it is failing asks "Leave without saving?".
20. Phone: the formatting bar (B, I, H, bullet, checklist, quote, link) sits above the keyboard while you write or edit, and its buttons format without closing the keyboard. The slash menu works with the keyboard. Back finishes the open edit instead of leaving the Journal; IME composition (a Gboard word suggestion, swipe typing) is not interrupted by pairs or the slash menu.
21. Checklists (dev browser, `/journal?seed=journal`): clicking anywhere on a checklist row in the read view ticks or unticks it without opening the editor, with a hover tint and a ringed box on desktop. Clicking the Entry's time, the space around the text or plain text opens the editor; a link still opens the browser. Ticking never adds "edited". Reload: the ticks stayed.
22. Entry actions: on desktop ⋯ appears on hover or keyboard focus of an Entry (on a phone it is always there); right-click (desktop) and a 500ms long-press (phone) open the same menu. It offers Edit, Move to another date…, Copy text ("Copied", Markdown on the clipboard) and, after a hairline, Delete in red. Esc or a click outside closes it and returns focus. The menu and toasts sit above the pinned header and the composer.
23. Delete: the Entry leaves at once with "Entry deleted" and Undo for 5 seconds; Undo puts it back in its original place on its Page. Deleting the only Entry of a Page removes the Page; undoing brings it back. Deleting an Entry that is open for editing closes the editor first, and its latest text comes back with Undo. Purge (real app): an Entry deleted over 30 days ago and its change-log records are gone after the next start (Rust test covers it).
24. Composer date chip (dev browser, `/journal?seed=journal`): the chip reads "Today ▾"; it opens the DatePicker (popover on desktop, bottom sheet on a phone) with Today and Yesterday chips above a month calendar. Pick a past day and write: the Entry lands on that Page when you finish ("Yesterday" and "Thu 15 Oct" labels; other years add the year), and the chip resets to Today. Pick a future day: the Entry goes on a future Page above Today. Changing the chip after you have started typing moves the Entry already saved.
25. Moving: the editor's bottom bar has a date chip showing the Entry's Page; ⋯ ▸ Move to another date… does the same. From ⋯, picking a date moves the Entry, the feed stays where it is, and a toast "Moved to Mon 19 Oct" appears with Show; Show scrolls to the Entry in its new place and flashes it in the soft coral tint (a steady tint, no fade, under Reduce motion). The Entry keeps its written time ("Mon 19 Oct" above the time when it was written on another day). Picking a date with the editor's own chip keeps the editor open on the Entry in its new Page, scrolled into view, with no Moved toast.
26. Future Pages: they sit above Today (newest first overall). The Journal opens scrolled to Today, not the top, with an "↑ Upcoming (n)" pill (n is the number of future Pages) that scrolls up to them; below one screen from Today the "↑ Today" pill replaces it and goes back to Today. Moving an Entry to a new future date adds its Page without moving what you were reading. New Entries still default to Today.
27. Calendar: the calendar button in the header (and ⌘G, Journal only) opens a popover on desktop and a bottom sheet on a phone on the month of the Page in view with that day highlighted; coral dots mark days with Entries (and update after a move, a delete or a new Entry); today has a ring. The week starts on the General Setting's day. ‹ › change month; the title opens a month and year picker (year arrows, 12 months); Today jumps to today's Page. Arrow keys move within the grid, PageUp/PageDown change month, Enter picks.
28. Calendar day taps: a day with Entries closes the calendar, scrolls to that Page and flashes its header. A far date does not load every Page in between: the feed restarts AT that date (only the Pages around it load), scrolling up brings newer Pages in above without the view jumping and scrolling down loads older ones; "↑ Today" (or ⌘N) drops that window and returns to Today. An empty day stays open and says "No entries on Thu 15 Oct" with Write on this day, which closes the calendar, sets the composer chip to that day and focuses the composer; the Entry then lands on that Page.
29. Dark theme and phone: the picker, calendar, chips and toasts sit above the pinned header and the composer; the dividers and today ring are visible in dark; the sheet is full width at 390px with 42px day cells. The Sheet has a Close button. On a phone (sandbox build) the SANDBOX badge sits next to the Journal title.
30. Search entry points: the header search button, ⌘F in the Journal (the browser's find does not open) and ⌘K. In ⌘K, "Search Journal" opens a blank search from anywhere; typing words adds "Search Journal for “…”", which opens the Journal with that query filled in and focus in the field. The composer hides while searching (its text is kept); closing search brings it back.
31. Search results (dev browser, `/journal?seed=journal`): results appear about 150ms after you stop typing, in place of the feed, grouped under small uppercase Page dates (the year only for other years), newest first, each with its time and a snippet with the matches tinted. Try `river` (exact), `RIVER`, `bakery` and a prefix `bake`, `cardamom bun` (all words, any order), `"first page"` (phrase), `bakry` (nothing exact, one entry under "Similar matches"), `zzz` ("No entries match “zzz”"). Words under 4 letters are never fuzzy (`cot` finds no `cat`). An empty field shows "Search your Journal".
32. Date filter: "Any date ▾" offers Past week / month / year and "Custom range…" (DatePicker: first day, then last day; a popover on desktop, a bottom sheet on a phone). The active range shows on the chip (tinted) with × to clear. It filters by Page date, narrows words, and with an empty field lists every Entry in the range. Choosing or clearing returns focus to the field.
33. Keyboard: ↓ / ↑ move a highlight through the results from the field (↑ from the first returns to the field), Enter opens the highlighted result (or the first), Esc clears the words, a second Esc closes search. Clicking a result does the same as Enter.
34. Opening a result closes search, scrolls to the Entry (restarting the feed at its Page when it is far away, as for the calendar) and flashes it. A "Back to results" pill appears; it, ⌘F (and Android Back) return to the same query, range and scroll place. × on the pill, Esc or the header search button leave search for good: the next search starts empty with no date filter. Leaving the Journal also clears it. With search parked (after opening a result), the header search button clears it for good.
35. Deleting or editing an Entry while its results are parked updates them when you return (no stale rows). Dark theme and phone: the field, chip, menu, picker, tint on matches and the pill are legible and sit above the feed; nothing scrolls sideways at 390px.
36. Real app: search finds Entries written before this version (the migration indexes them), ignores accents (`cafe` finds `café`), never returns a deleted Entry (and finds it again after Undo), and quotes, `*`, `-`, `OR`, `NEAR(` or `:` in the query never show an error.
37. Keyboard navigation (desktop, `/journal?seed=journal`): Tab onto an Entry or press ↑ from the empty composer; a clear focus ring (and tint) shows on the Entry, in light and dark, and it is not the editor's card. ↑ / ↓ move between Entries across Pages in feed order and scroll the Entry into view below the pinned header and above the composer fade; ↓ past the last loaded Entry loads older Pages and carries on. ↑ from an empty composer lands on the last Entry of Today's Page (the newest earlier Entry when Today is empty); with text in the composer ↑ just moves the caret. The keys do nothing while typing in the editor, the composer or the search field.
38. On a focused Entry: Enter opens the editor; Esc, ⌘↵ or Done closes it with the Entry still focused, and ↓ carries on from it (a click outside does not refocus it). ⌫ / Delete deletes with Undo and focuses the next Entry (the composer when none is left); Undo brings it back. M opens the move picker (Esc returns focus to the Entry); C copies ("Copied"); Esc on the Entry returns to the composer. ⌘⌫ does nothing on an Entry. The ⋯ menu shows ↵, M, C and ⌫ as hints on desktop and not on a phone; Settings ▸ Shortcuts lists the Journal keys.
39. ⌘N from Home, from Settings, with search open and with another Entry open for editing: the Journal opens on Today (scrolled there), the composer is focused and its chip reads Today (text already being written keeps its date). From the Journal itself it scrolls back to Today. ⌘K: "New journal entry" does the same; "Go to date…" opens the Journal with its calendar open; "Search Journal" and "Search Journal for “…”" still work. The palette lists them under Journal with ⌘N, ⌘F and ⌘G.
40. Home card (dev browser, `/?seed=journal`): "Journal" with the coral notebook icon, the latest Entry written today as a plain-text quote in the serif (no Markdown symbols, at most 3 lines, about 140 characters), "Today · N entries" and a Write button that opens the Journal with the composer focused. With no Entries today (`/` with no seed, or a seed with none today) it says "Nothing written today" plus Write. Write an Entry, finish it and go back to Home: the card shows it and the new count. Switching the Journal off removes the card.
41. Phone tabs: a fresh install (no saved choice) shows Home and Journal tabs; Settings ▸ Navigation shows the same two. An install from before the Journal that never saved its tabs keeps Home only (a Rust test covers it); a saved choice is untouched.
42. Settings ▸ Journal (also after the core sections on a phone): Writing font Serif / Sans, Text width Narrow / Default / Wide, Page summaries and Spellcheck. Changing the first three marks Settings unsaved (SaveBar); the Journal does not change until Save, then at once: the Entries, composer and editors switch font; the feed, pinned header, composer and search share the new width (about 60 / 72 / 90 characters; all fill a narrow window and grow with Text size); Page summaries off hides "N entries · N words". Discard reverts. Spellcheck saves immediately (no SaveBar): in the real app a misspelled word is underlined in the composer and an open editor only while it is on, and it stays off after a restart. It is per device and never syncs.
43. Real app: the Journal Preferences and Spellcheck survive a restart; `device.toml` holds `spellcheck`, the database holds the three Preferences.
44. Composer leak: start a new Entry and keep typing, then trigger a feed refresh (finish another Entry, or change the date chip): the in-progress Entry never appears in the feed until you finish it.

## Performance budgets

- First-load JS stays small: lazy-load route components (the registry already does) and heavy libraries (state machines, charts, CodeMirror: `MarkdownEditor` imports it dynamically).
- Fonts: Latin subset only, variable files from `@fontsource-variable`.
- **No polling.** Rust events invalidate queries (one event bridge). Nothing runs on an interval to check for changes.
- **One timer rule**: time-driven UI shares the single clock in `src/lib/time/clock.svelte.ts` (one second-aligned timer, active only while subscribed and the window is visible). Do not add other `setInterval`s.
- Lists beyond a few hundred rows use `@tanstack/svelte-virtual`.
- **No `VACUUM`** without rebuilding full-text indexes afterwards: they are keyed on implicit rowids (the Journal's `journal_fts`), which VACUUM may renumber. Backups use the page-copy backup API, which keeps rowids.
- Animate only `transform` and `opacity`; durations come from tokens and are zero under reduced motion.

## Resource rules for the dev machine

The dev machine is an 8 GB Mac with limited disk.

- Always `CARGO_BUILD_JOBS=2` (the Makefile sets it). One cargo build at a time.
- No language servers (rust-analyzer, Serena). Use search and read the code.
- Prefer `cargo check -p <crate>` over a full build while iterating.
- Run at most one dev server. Kill what you start (`pgrep -fl vite`, then kill your own PIDs), and leave other projects' processes alone.
- No browsers or emulators on this machine; test on a real phone via adb.
- `make clean-heavy` frees `target/`, `node_modules/` and Android build output.

## Android project

`src-tauri/gen/android` is committed and customised. Do **not** re-run `cargo tauri android init`. The customisations are:

- AGP 8.13.0 and the Gradle 8.14.3-all wrapper, chosen to reuse the local cache.
- arm64 only (`abiFilters`, `RustPlugin.kt` defaults); R8 off.
- Release signing (fails loudly if not set up) and the Sandbox switch (`MNEMORA_ANDROID_SANDBOX=1` gives `com.hemang.mnemora.sandbox` and the "Mnemora Sandbox" label). The Tauri identifier stays the Production one on Android (the CLI requires it to match the Kotlin package); `src-tauri/src/env_guard.rs` checks the package via the data dir instead.
- Memory limits in `gradle.properties`.
- The `MainActivity` system-bars bridge.
- The vector adaptive launcher icon in `res/drawable` + `res/mipmap-anydpi-v26`.

## Icons

All icons (macOS, Windows, Linux, iOS, Android, favicon) are generated by `make icons` from the SVG masters in `src-tauri/icons/source/`. Edit the SVGs, never the generated files. Rasterize only with `cargo tauri icon` (resvg): `qlmanage` paints an opaque white background behind transparent margins. Desktop icons are embedded into the binary at compile time; `src-tauri/build.rs` tracks `icons/` so a `make icons` run always triggers a rebuild.

If it is ever re-run, re-apply these with `git diff` / `git checkout -p`.

### Todo

1. ⌘3 opens Todo; it sits right after the Journal in the sidebar (and can be switched off and on in Settings ▸ Modules). Todo opens on Today; click Inbox under Todo. The page title reads "Inbox" with the teal Task icon (desktop top bar; the large title on a phone). A new database shows "+ New task" and "Nothing here. Tasks without a List land in the Inbox."
2. Click "+ New task": it becomes an inline field in place, focused. Type a title and press Enter: the Task appears above the field, the field stays open and empty for the next, and a long list scrolls to keep it in view. Press Enter on an empty or blank field: nothing happens.
3. Esc closes the field and puts focus back on "+ New task"; clicking anywhere else closes it too. Anything typed and not added is dropped. A field cannot take more than 500 characters or a line break; in the real app a title Rust rejects keeps its text in the field and shows the reason in a toast.
4. New Tasks always go to the bottom, in the order they were added; reload (or restart the app) and the order is the same.
5. The Task checkbox is a diamond (a rounded square turned 45°, tick upright): faint edge, teal edge and halo on hover, teal fill and white tick when done; keyboard focus shows the soft halo ring. On a phone it is bigger with a larger touch area. Checklist squares inside Journal Entries are unchanged.
6. Tick a diamond: the Task leaves the Inbox at once and a toast `Completed “Buy oat milk”` offers Undo for 5 seconds (long titles are cut at 34 characters with an ellipsis). Undo returns the Task to its old place, not the bottom. Ticking several in a row stacks the toasts (max 3).
7. Real app: the done Task keeps its done time (`closed_at`) in the database; every add and tick adds a `todo_task` record to the change log, and no Task title appears in the log files.
8. Real app: starting a build that has Journal data but no Todo tables backs the database up first, then adds the Todo tables; the Journal is untouched.
9. Nested sidebar: entering Todo (⌘3 or the row) unfolds Inbox, Today, Upcoming, All, Done and a "Lists" header with + under it; the open view carries the active tint and the Todo row is bold, not tinted. Click Home or the Journal: the tree folds. Come back: it unfolds. While in Todo, clicking the Todo row or its chevron folds and unfolds it; go to Home and back: the automatic rule applies again whatever you did by hand. Quit and reopen (real app): the tree is as you left it. Done shows a placeholder but routes and shows active; Todo reopens on the view you last used until you quit the app (a launch starts on Today).
10. Rail: collapse the sidebar (⌘\). Hover the Todo icon for 600ms or right-click it: a flyout lists the five views and the Lists with open counts (Inbox too). Picking one opens it and closes the flyout; moving the pointer away closes it; clicking the icon goes to Todo. The flyout never takes focus from a field.
11. New list: click + on the Lists header; an inline name field appears focused. Enter creates the List at the bottom and opens it (title and "+ New task" ready); Esc or clicking away drops it. A name equal to another List's ignoring case and accents ("home" vs "Home", "Café" vs "cafe") keeps the field open with "There’s already a List with that name"; typing clears the message. Blank names do nothing; the field takes at most 60 characters.
12. List page: click the title to rename it in place (Enter saves, Esc cancels, a taken name shows the message under the field); click the emoji (or the neutral list icon) to open the picker: a grid of 48, "Type or paste any emoji" (picks as soon as one appears) and "No emoji" to clear. The sidebar and flyout follow. A List without an emoji shows the neutral list icon.
13. Sidebar List menu: right-click a List (long-press on a phone): Rename (inline in the sidebar), Emoji… (the picker next to the row) and, after a hairline, Delete list… in red.
14. Delete an empty List: it goes at once with "Deleted “Name”" and Undo, which brings it back (and reopens it if it was open). Delete a List with Tasks (done ones count too): a Dialog asks; "Delete the Tasks too" is the default, "Move to…" shows a menu of Inbox and every other List. Each choice ends with a toast saying what happened to the N Tasks and Undo, which restores the List and puts every Task back (deleted ones return, moved ones go home). If the deleted List was open, Todo lands on the Inbox. Undo fails with a message when a new List took the name meanwhile.
15. Counts: the sidebar shows the open count beside the Inbox and each List (nothing when zero); ticking, adding, moving and deleting update them. The Inbox row and the Inbox group are always first and have no rename or delete.
16. All view: open Tasks grouped under a heading per List (emoji, name, count), the Inbox first; empty groups are left out; clicking a heading opens that List; "No open Tasks." when there are none. Ticking works here too.
17. List page "+ New task" adds into that List (the Inbox "+ New task" still adds to the Inbox); the new Task shows in the List, in All and in the count.
18. Task menu: hover a row for ⋯ (not shown on a phone), or right-click it (long-press on a phone): "Move to list…" opens a picker (a popover next to ⋯, a bottom sheet on a phone) of Inbox and every List with a tick on the current one. Picking moves the Task and a toast "Moved to “Home”" offers Undo, which moves it back. Real app: each move adds a `move` record to the change log.
19. Real app: every List write (create, rename, emoji, delete, restore) adds a `todo_list` record to the change log, deleting a List's Tasks adds `delete` records and restoring them `restore` ones, and no List or Task name appears in the log files. `device.toml` holds `sidebar.todo_open` once you have folded or unfolded by hand or navigated (until then a hard load of `/todo` shows the tree open).
20. Dark, True black and a 360px-wide window (the sidebar is not shown on a phone yet; Todo's phone navigation arrives with the phone ticket): the tree rows, flyout, emoji picker, delete Dialog and List picker are legible and sit above the page.
21. Dates in rows (dev browser; create Tasks with dates by hand or with `__TAURI_INTERNALS__.invoke`): a row shows its date under the title as Today (teal), Tomorrow, a weekday within the week ahead (Fri), then 28 Oct, with the year when it is not this year; an Overdue date is red with a red diamond edge; a time shows with a clock; a Task due today whose time has passed stays in Today with its time in red. Times follow the 12h/24h Setting.
22. Hover a row (desktop): ☀ Today and 📅 When… appear beside ⋯. ☀ makes it due today (keeping its time) with a toast `“Title” → Today` and Undo, which puts the old date back even when it was in the past. ⋯ and right-click show "When…" above "Move to list…"; both open the same picker.
23. When picker (desktop popover, about 420×280): the "Type a date" field is focused. Quick days show their weekday on the right: Weekend is today on a Saturday or Sunday, otherwise the coming Saturday; Next week is the first day of next week (switch the Week start Setting to Sunday to see it move). The calendar starts at this week: past days are faint and not clickable, today is ringed, the 1st carries a tiny month label, ‹ is disabled on the first page and › moves five weeks; days with Tasks have a dot, amber at three or more (the Task being dated is not counted). Clicking a day sets it and closes the picker; the Time chip keeps what it had.
24. Type `fri`, `28 oct`, `in 3 days`, `tomorrow 9am`, `9am`: the right of the field shows how it was read ("Fri 23 Oct · 09:00 ↵") and Enter applies it and closes; something it cannot read says "Not understood". `18 oct` means next year; `2/3` is never read.
25. Time chip: opens a panel over the calendar; nothing is suggested until you type. `9` offers 09:00 and 21:00, `930p` 21:30, `21:30`, `noon`; Enter takes the first, a click takes any, "Remove" clears the time. A time on a Task with no date dates it today if the time is still ahead, otherwise tomorrow. Esc closes the panel first, a second Esc closes the picker. No native date or time inputs anywhere.
26. Phone (360px, the picker is a bottom Sheet with Close): the same layout fits with no sideways scroll; cells are touch sized and the weekday hints are left out.
27. Today: a large serif "Today" heading in the page (desktop; a phone has the large title above) with the full date beside it, smaller and muted ("Today  Wednesday, 21 October"), and at the right of that line a thin progress bar and "N of M done" (hidden when nothing is due or done). The sidebar's Today row (and the rail flyout) shows its count, open Tasks due today plus Overdue, in red while anything is Overdue; Inbox keeps its own count. A red Overdue group sits on top with "Move all to…" and "Move all to Today". Under it come today's Tasks (timed ones first by time, then manual order; the group is headed "Today" only while Overdue shows), "+ New task" (adds a Task due today) and "Completed today" with struck-through Tasks and their time done; ticking one of those brings it back. An empty day says "Nothing due today."
28. Move all: "Move all to Today" and "Move all to…" (the picker without Time or No date) move every Overdue Task, keeping times; the toast `Moved 3 Tasks to Today` offers Undo, which puts each Task back on its own past date. Each Task's change is its own `update` record (real app). Overdue Tasks never move by themselves.
29. Upcoming: only days with Tasks, under serif day headers (Tomorrow, Friday) with a small date (and the year when not this year); Overdue on top with the same Move all; each day ends with "Add to this day", which creates a Task due that day. The week strip stays pinned while you scroll: dots on days with Tasks, today and past days disabled, ‹ › move by a week (‹ disabled on this week), clicking a day scrolls to it, and an empty day appears with an add row while it is selected (click it again, or pick another, to let it go).
30. Midnight (dev browser: run `window.Date = class extends Date { constructor(...a) { super(...(a.length ? a : [Date.now() + 864e5])) } static now() { return Date.now() + 864e5 } }` in the console, or leave the app open across midnight): within a second Today, Upcoming, the picker's ring and every row's labels move to the new day and yesterday's Tasks are Overdue. Nothing polls: the shared clock only ticks while Todo is open.
31. Real app: setting, clearing and moving dates each add an `update` record to the change log (an unchanged date adds none); a past date is refused with "That date is in the past" and nothing is written; no Task title appears in the log files.
32. Repeat panel (desktop, dev browser): the When picker's bar has a Repeat chip beside Time; it opens a panel over the calendar with Every day, Every 2 / 3 / 4 days, Every weekday, Weekly on ‹weekday›, Monthly on the ‹date›, Yearly on ‹date› in two columns, "Custom…" and "Remove" in the header, "From the due date / From the day it's done" and "Next:" with three dates below. Picking a preset sets it (the chip names it, the row shows `↻ Every 3 days`), the panel stays open, the days it lands on carry a fainter ring in the grid, and the toast `“Title” → Thu 22 Oct · Every weekday` offers Undo (changing it again replaces the toast). Esc closes the panel first, a second Esc the picker.
33. Custom…: "Every" takes 1 to 99 (↑ ↓ step it; typing 0 or 100 leaves the last good value); Day / Week / Month / Year; Week shows M T W T F S S toggles (none chosen means the due date's own weekday). "From the day it's done" is disabled while weekdays are chosen and switching it on clears nothing; choosing a weekday while it is on switches back to "From the due date". `Every 2 weeks on Mon, Thu` rings Mon and Thu two weeks apart.
34. Setting a Repeat on an undated Task dates it: "Every weekday" on Wednesday gives Today, "Mon and Thu" gives Thursday. Removing the date (No date, or the date's Undo) removes the Repeat; Undo brings the date, time and Repeat back together. Moving the date by hand keeps the Repeat and makes the new date its anchor (monthly on the 31st, moved to the 5th, is monthly on the 5th).
35. Ticking a repeating Task: it does not leave the app. The toast reads `Completed “Water the plants” · next Sat 24 Oct` with Undo; the Task shows its new date (time kept), a done copy appears in Done (and under "Completed today" in Today) marked `↻ occurrence`. Undo deletes the copy and puts the Task back on its old date. A Repeat counted from the due date goes to the first date after today when ticked late and one step after the due date when ticked early; "from the day it's done" counts from today.
36. Reopening from Done: the newest occurrence of a Task rewinds it to that date (the copy leaves Done); an older one shows the toast "Only the latest occurrence can be reopened". The row menu of a repeating Task offers `Skip to Sat 24 Oct`: it moves on with the toast `Skipped “…” · next …` and Undo, and nothing appears in Done.
37. Done: grouped under a serif heading per day (Today, Yesterday, the weekday, then the small date and a count), newest day first and newest first within a day; rows show the done time and List. Scrolling to the end loads another 14 days with no button; "That is everything." closes the list; an empty Done says "Nothing done yet". Phone (360px): the same, with no sideways scroll; the Repeat panel in the Sheet fits without scrolling (presets and Custom). Light and dark themes.
38. Real app: ticking a repeating Task adds a `create` (the copy) and a `tick` (the Task) to the change log, reopening adds a `tick` and a `delete`, skipping an `update`; Comments of the occurrence move with it; no Task text is in the log files.
39. Task sheet (desktop, dev browser): click a row's title, press Enter on it, or use ⋯ ▸ Open. A centred sheet opens and the address becomes `/todo?task=‹id›`: the diamond (its ring shows the Priority) and the title, the chips List · When · Priority, the notes in the live editor, Comments with a composer, History with its latest line, and a footer `Created Wed 14 Oct · Done Today 09:12`. Esc, a click outside, ✕ and ⌘↵ all close it and the address goes back to `/todo`; browser Back closes it and Forward reopens it. Paste a `/todo?task=‹id›` address (or reload it in the real app): the sheet opens on that Task; an id that no longer exists shows "That Task no longer exists" and closes. On a phone it is a bottom sheet with a grab bar and Android Back closes it.
40. Editing: the title is raw text (Markdown marks visible); Enter moves into the notes and never adds a line break; clear it and click elsewhere and the previous title comes back; a title over 500 characters cannot be typed. Notes are the Journal's editor in the Journal's Writing font (switch Settings ▸ Journal ▸ Font to check); `- [ ]` lines show as square boxes (not diamonds): click one to tick it (autosaved; the row's `1/3` updates; ticking the last one does not complete the Task); Enter on a Checklist line continues the list. While the title is not focused it shows its `**bold**`, links and code; click it (not a link) to edit the raw text, and a link click opens the browser. Everything autosaves 600 ms after the last keystroke and again when the sheet closes: close it right after typing and the row behind shows the new title, notes preview and "1/3" at once; no Save button exists. A failed save keeps the text and shows "Couldn't save this Task. Your text is safe here." with Retry.
41. ⌘↑ / ⌘↓ (Ctrl on Windows and Linux) switch the open sheet to the previous or next Task of the list behind it (Today, an Upcoming day, a List, All, Done), also while the notes have focus; the first and last Task do nothing; a Task completed from the sheet can still be reached. Moving does not add history entries (Back still closes the sheet in one step).
42. Chips: the List chip opens the List picker (a popover; a bottom sheet over the sheet on a phone) and moves the Task with the usual `Moved to “…”` toast; the When chip opens the When picker and reads `Tomorrow · 09:00 · Every month` (teal; red when Overdue; "When" while undated); the Priority chip opens a menu with P1 red, P2 amber, P3 violet and No priority, the current one ticked, and reads `P1` (tinted) or "Priority". On a Done or Won't do Task the When and Priority chips are dimmed and do nothing. A change shows its toast naming the Task (`“Pay rent” → P1`) with Undo.
43. Priority keys: hover a row (or Tab onto it) and press 1 to 4; with the sheet open the keys set the sheet's Task (also when focus is on a chip or the sheet itself). They do nothing while typing in a field (the New task field, the comment composer, the title, the notes), inside a menu or picker, or on a done Task. The diamond shows a red, amber or violet ring for P1 to P3 and nothing for P4; ⋯ ▸ Priority… opens the same menu. In Today and Upcoming, inside each time block (the same time, or no time) the more urgent Task comes first; a timed Task still comes before an untimed one.
44. Open steps: tick the diamond of a Task whose notes hold unticked `- [ ]` items (row, Today, the sheet): a Dialog `2 steps are still open` offers Cancel (nothing happens), "Tick all and complete" (the notes get every step ticked, then it completes) and Complete (the steps stay unticked); it asks every time, also right after Undo. A Task with no steps, or all ticked, completes without asking. On a repeating Task the next occurrence starts with every step unticked either way.
45. Won't do: ⋯ ▸ Won't do or ⇧Space on a hovered or focused row (or the sheet's Task) closes the Task with `Won’t do “…”` and Undo; Done lists it struck through with a grey ⊗ diamond and `Won’t do · 19:02`, never counted in Today's "N of M done" or the sidebar. On a repeating Task the same item and key read "Skip to Sat 24 Oct" and skip it instead. Its diamond reopens it. ⇧Space while typing types a space.
46. Delete: ⋯ ▸ Delete (row or sheet footer ⋯) removes the Task at once with `Deleted “…”` and Undo, which puts it back in the same place; deleting from the sheet closes it first. Deleting a repeating Task keeps its occurrences in Done. Real app: a record `delete` / `restore` is added to the change log; Tasks, Lists and Comments deleted more than 30 days ago are erased at the next start with their change-log records (check `SELECT COUNT(*) FROM change_log WHERE entity = 'todo_task'` before and after by backdating `deleted_at` on a test row).
47. Comments: the composer adds one on Enter (Enter posts, Shift+Enter adds a line break, up to 5,000 characters; inline marks `**bold**`, `*italic*`, `` `code` ``, links and bare URLs render); newest at the bottom. Hover a comment (always visible on a phone) for ✎ and 🗑: ✎ edits in place (Enter saves, Esc cancels, blur saves) and the stamp reads `Today 09:12 · edited`; 🗑 deletes with `Comment deleted` and Undo, which restores it in place. The count shows beside "Comments" and as 💬 2 on the row. Complete a repeating Task: its comments move to the occurrence in Done (open it there to see them) and the Task starts clean; reopening that occurrence hands them back. Clicking a toast's Undo while the sheet is open does not close the sheet.
48. History: the fold shows the latest line; "Show all N" unfolds the rest, newest first, each with a stamp (`Today 09:12`, `Mon 08:00`, `14 Oct 08:00`): `Created`, `Due date Thu 22 Oct → Fri 23 Oct`, `Time set to 09:30`, `Repeat set: Every 3 days`, `Priority P4 → P1`, `Moved 📥 Inbox → 💼 Work`, `Done`, `Won’t do`, `Reopened`, `Deleted`, `Restored`, `Done (occurrence of 18 Oct)`. Typing in the title and notes through one opening of the sheet makes one `Title edited` and one `Notes edited` line, even when you change the Priority or date in between; closing and reopening starts another. Nothing in History can be edited.
49. Rows: a row shows the first line of prose of the notes (marks removed, Checklist lines skipped; nothing when the notes are only a Checklist), `1/3` with a list-check icon, and 💬 with the comment count, after the date, time and Repeat. Titles render `**bold**`, `*italic*`, `` `code` ``, `[links](https://…)` and bare URLs (a link opens the system browser and never opens the sheet); `# heading`, `- item`, `> quote` and `1. item` stay as typed.
50. Real app: setting a Priority, Won't do, delete and restore each add one `update`, `tick`, `delete` or `restore` record; all the autosaves of one opening of the sheet make a single `update` record (check `session_id` is shared and the count does not grow while typing); Comments add `todo_comment` records. No record or log line holds Task, note or comment text beyond the snapshot columns of the change log itself.
51. Dark, True black and a 360px-wide window: the sheet, chips, Priority menu, comments, History, the steps Dialog (its three buttons wrap), the ⊗ rows in Done and the row meta line show no clipped text or sideways scroll; focus rings show on keyboard focus only (not around the sheet itself).

## How to add a Module

Example id: `journal`.

**Rust**

1. Create `crates/journal` (`Cargo.toml` using `workspace = true` lints and dependencies, depend on `mnemora-core`). Add it to the workspace `members` in the root `Cargo.toml`.
2. Implement the contract from `crates/core/src/module.rs`:
   ```rust
   pub struct JournalModule;
   impl Module for JournalModule {
       fn id(&self) -> &'static str { "journal" }
       fn migrations(&self) -> Migrations<'static> { /* M::up(...) list */ }
       fn register_settings(&self, reg: &mut SettingsRegistry) { /* reg.register("journal", key, default, validator) */ }
       // Optional: fn on_startup(&self, conn: &mut Connection) -> AppResult<()>, run once per start after migrations.
   }
   ```
3. Migrations are append-only and tracked per Module. Tables use a UUIDv7 text id, `created_at`/`updated_at`, and a nullable `deleted_at` for soft delete ([ADR 0003](../adr/0003-sync-ready-sqlite.md)).
4. Put business logic and validation in the crate; test the rules through the commands with `TestApp`, and keep pure-function unit tests in the crate. If the browser mock mirrors a rule, add a shared fixture case. Talk to other Modules only through core contracts and `EventBus` events.
5. Register it in `src-tauri/src/lib.rs` in `modules()` (the one place). Add its Tauri commands to `specta_builder()` (commands stay thin and call the crate), then run `make bindings`.

**UI**

6. Create `src/modules/journal/` with `manifest.ts` exporting a `ModuleManifest` (`id`, `name`, Lucide `icon`, `accentVar` such as `--hue-journal`, `routes` with lazy `component: () => import('./JournalPage.svelte')`, `nav: { order, shortcut }`).
7. Add `JournalPage.svelte` (start it with `<PageHeader title="Journal" />`), and `queries.ts` with TanStack Query definitions that call `commands.*` through `unwrap`. Mutations invalidate by query key; Rust events already invalidate Settings.
8. Build screens only from `src/lib/ui`. Settings for the Module go in a Settings section that follows the section/row/save-bar pattern; forms use the SaveBar (long-form writing autosaves instead).
9. Register the manifest in `src/shell/registry.ts`. Routes, sidebar entry, tab chooser, palette and shortcuts follow from the manifest, and only while the Module is enabled. Optional manifest hooks: `homeCard` (lazy Home card), `settingsSection` (replaces step 8's hand wiring), `create` (⌘N) and `paletteActions`.
10. Add a hue token if the Module needs one (`--hue-journal` exists already), unit-test the logic, run `make lint test`, then update the glossary or add an ADR if you settled a term or decision.
