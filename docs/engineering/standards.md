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
- No E2E for now ([ADR 0006](../adr/0006-no-e2e-tests.md)). Use the manual smoke checklist instead: `make dev`, check theme, sidebar, Settings save/discard, restart persistence.
- `make test` and `make lint` must pass before you finish.

## Performance budgets

- First-load JS stays small: lazy-load route components (the registry already does) and heavy libraries (state machines, charts).
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
9. Register the manifest in `src/shell/registry.ts`. Routes, sidebar entry, tab chooser, palette and shortcuts follow from the manifest.
10. Add a hue token if the Module needs one (`--hue-journal` exists already), unit-test the logic, run `make lint test`, then update the glossary or add an ADR if you settled a term or decision.
