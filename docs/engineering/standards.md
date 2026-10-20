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
- Each crate maps its own errors with `thiserror`, then into `AppError` via `From`. Choose the narrowest `ErrorCode` (`validation`, `unknown_key`, `io`, `database`, `internal`).
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
- Rust: unit tests beside the code in `#[cfg(test)]`; database behaviour is tested against an in-memory SQLite (`db::open_in_memory`) or a `tempfile` dir.
- UI: Vitest, with `*.test.ts` next to the source. Keep logic in plain `.ts` so it is testable without a DOM.
- No E2E for now ([ADR 0006](../adr/0006-no-e2e-tests.md)). Use the manual smoke checklist instead (below).
- `make test` and `make lint` must pass before you finish.

## Manual smoke checklist

`make dev`, then:

1. Theme, density and sidebar (expanded and rail) look right; Settings save/discard works; values persist after a restart.
2. Home shows the greeting and clock, plus an empty state while no Module provides a card; Module cards appear for enabled Modules only.
3. Settings ▸ Modules: Home and Settings are checked and disabled. Switching a Module off and saving removes it from the sidebar, phone tabs and More, ⌘K and ⌘-digit; its URL falls back to Home. Switching it on and saving restores all of them. Unsaved switches show the leave prompt.
4. A Module's Settings section appears after the core sections (and drills in on phone) only while the Module is on.
5. ⌘N runs the first enabled Module's create action; with none it shows "Nothing to create yet".

### Journal

1. ⌘2 opens the Journal; it also appears in the sidebar (and can be switched off and on in Settings ▸ Modules). An empty Journal shows "A blank page".
2. Desktop: the composer ("Write…") is focused on open; on a phone it is not. It is as wide as the text column.
3. Type one character: exactly one Entry is created (check again after typing more; no second Entry). Pause 600ms and keep typing: the same Entry updates. A blank or whitespace-only composer never creates anything.
4. Finish with ⌘↵ / Ctrl+↵, Esc and the ✓ button (one each): the composer clears and the Entry appears on Today's Page with its time and line breaks kept.
5. Quit and reopen the app: the Entries are still there, newest Page first, oldest Entry first within a Page.
6. Leaving the Journal mid-sentence still saves the text.
7. No Entry text appears in the log files (Settings ▸ Data & storage ▸ open the log folder).
8. Feed: each Page header shows the written date in the serif ("Tuesday, 20 October") with a coral Today/Yesterday badge, the year only for other years, and (desktop only) "N entries · N words" with singular/plural. The header stays pinned while you scroll its Page. A faint uppercase month divider appears where the month changes. Dates with no Entries are skipped.
9. Entries: the time sits in a left margin on desktop and above the text on a phone; a hairline separates Entries; text is serif with blank-line paragraph gaps. An Entry written on another day than its Page shows a short date (stacked above the time on desktop, on the time line on a phone), with the year if it differs. Hovering the time shows "Written Monday, 19 October 2026 at 09:12 (Asia/Kolkata)". An edited Entry shows a faint "edited" (hover for the time). Switching 12h/24h in Settings changes the times.
10. Scroll a long Journal (with many Pages): older Pages load before you reach the end with no button; scrolling stays smooth. After about a screen, "↑ Today" appears; it scrolls back smoothly (instantly under Reduce motion) and disappears at the top.
11. An empty Today with older Pages shows "Nothing yet today. The box below is ready."; a brand-new Journal shows the coral notebook icon, "A blank page" and "Start typing in the box below". A slow load shows a skeleton only after 300ms; a failed load shows "The Journal couldn't load." with Retry and Copy details.

12. Markdown reading: an Entry renders without symbols: headings (serif, heavier), bold, italic, strikethrough, highlight, quote with a bar, divider, bullet/numbered/checklist (checkbox look), inline code, fenced code, links and bare URLs (click opens the browser). A table stays as plain text. Open `/journal?seed=journal` in the dev browser to see every feature in the first Entry.
13. Inline editing: click an Entry (not a link) to edit it in place: it lifts onto a card, the rest dims to about 35%, and the text does not shift apart from the symbols. Esc, ⌘↵, a click outside and Done each finish; ⌘Z / ⌘⇧Z undo only inside the open editor. Edits within 10 minutes of writing do not show "edited"; later ones do. Quit mid-edit and reopen: the text is there.
14. Markdown symbols: while editing, `**`, `#`, `>`, `-`, backticks and link addresses stay visible but faded.
15. Shortcuts (⌘ is Ctrl off macOS): ⌘B bold, ⌘I italic, ⌘⇧X strikethrough, ⌘⇧H highlight, ⌘E code, ⌘K link with a selection (with none, the palette opens), ⌘⇧8 / 7 / 9 bullet / numbered / checklist, ⌘⌥1–3 headings.
16. Slash menu: `/` at a line start or after a space lists H1–3, bullet, numbered, checklist, quote, divider, code block, today's date and the current time (in the General Settings formats). Arrow keys, Enter, Tab and click pick; Esc or a space closes it and leaves the `/` ("and/or" is plain text). A block item picked mid-line starts on a new line.
17. List keys and pairs: Enter continues a list and ends it on an empty item; Tab / ⇧Tab nest and lift; Backspace right after a marker removes it. Typing `(`, `[`, a backtick or `**` closes the pair; a selection is wrapped; typing the closer steps over it; Backspace between an empty pair removes both. Quotes and underscores are never closed. No smart quotes or dashes.
18. Paste: rich text from a web page keeps bold, italic, links, lists, headings and quotes as Markdown; ⌘⇧V pastes plain text. ⌘/Ctrl-click a link while editing opens it; a plain click places the cursor.
19. Save failure (dev browser): open `/journal?seed=journal&fail=journal_update`, edit an Entry and pause: the banner "Couldn't save this entry. Your text is safe here." appears with Retry and Copy details and the text stays. In the console run `__journalMock.failUpdate = false`, then Retry or type on: it saves and the banner goes. Leaving the Journal while it is failing asks "Leave without saving?".
20. Phone: the formatting bar (B, I, H, bullet, checklist, quote, link) sits above the keyboard while you write or edit, and its buttons format without closing the keyboard. The slash menu works with the keyboard. Back finishes the open edit instead of leaving the Journal; IME composition (a Gboard word suggestion, swipe typing) is not interrupted by pairs or the slash menu.
21. Checklists (dev browser, `/journal?seed=journal`): clicking anywhere on a checklist row in the read view ticks or unticks it without opening the editor, with a hover tint and a ringed box on desktop. Clicking the Entry's time, the space around the text or plain text opens the editor; a link still opens the browser. Ticking never adds "edited". Reload: the ticks stayed.
22. Entry actions: on desktop ⋯ appears on hover or keyboard focus of an Entry (on a phone it is always there); right-click (desktop) and a 500ms long-press (phone) open the same menu. It offers Edit, Copy text ("Copied", Markdown on the clipboard) and, after a hairline, Delete in red. Esc or a click outside closes it and returns focus. The menu and toasts sit above the pinned header and the composer.
23. Delete: the Entry leaves at once with "Entry deleted" and Undo for 5 seconds; Undo puts it back in its original place on its Page. Deleting the only Entry of a Page removes the Page; undoing brings it back. Deleting an Entry that is open for editing closes the editor first, and its latest text comes back with Undo. Purge (real app): an Entry deleted over 30 days ago and its change-log records are gone after the next start (Rust test covers it).

## Performance budgets

- First-load JS stays small: lazy-load route components (the registry already does) and heavy libraries (state machines, charts, CodeMirror: `MarkdownEditor` imports it dynamically).
- Fonts: Latin subset only, variable files from `@fontsource-variable`.
- **No polling.** Rust events invalidate queries (one event bridge). Nothing runs on an interval to check for changes.
- **One timer rule**: time-driven UI shares the single clock in `src/lib/time/clock.svelte.ts` (one second-aligned timer, active only while subscribed and the window is visible). Do not add other `setInterval`s.
- Lists beyond a few hundred rows use `@tanstack/svelte-virtual`.
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
   }
   ```
3. Migrations are append-only and tracked per Module. Tables use a UUIDv7 text id, `created_at`/`updated_at`, and a nullable `deleted_at` for soft delete ([ADR 0003](../adr/0003-sync-ready-sqlite.md)).
4. Put business logic and validation in the crate, with in-memory DB tests. Talk to other Modules only through core contracts and `EventBus` events.
5. Register it in `src-tauri/src/lib.rs` in `modules()` (the one place). Add its Tauri commands to `specta_builder()` (commands stay thin and call the crate), then run `make bindings`.

**UI**

6. Create `src/modules/journal/` with `manifest.ts` exporting a `ModuleManifest` (`id`, `name`, Lucide `icon`, `accentVar` such as `--hue-journal`, `routes` with lazy `component: () => import('./JournalPage.svelte')`, `nav: { order, shortcut }`).
7. Add `JournalPage.svelte` (start it with `<PageHeader title="Journal" />`), and `queries.ts` with TanStack Query definitions that call `commands.*` through `unwrap`. Mutations invalidate by query key; Rust events already invalidate Settings.
8. Build screens only from `src/lib/ui`. Settings for the Module go in a Settings section that follows the section/row/save-bar pattern; forms use the SaveBar (long-form writing autosaves instead).
9. Register the manifest in `src/shell/registry.ts`. Routes, sidebar entry, tab chooser, palette and shortcuts follow from the manifest, and only while the Module is enabled. Optional manifest hooks: `homeCard` (lazy Home card), `settingsSection` (replaces step 8's hand wiring), `create` (⌘N) and `paletteActions`.
10. Add a hue token if the Module needs one (`--hue-journal` exists already), unit-test the logic, run `make lint test`, then update the glossary or add an ADR if you settled a term or decision.
