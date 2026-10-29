# Mnemora

Mnemora is an offline-first life-management app for desktop and Android, built with Rust and Tauri 2.
It is a set of independent Modules (Journal, Todo, AI, ...) that share one shell, one look and one Settings page.
Your data stays on your device in a single SQLite file.

## Requirements

- Rust (stable, 1.90+) and `cargo install tauri-cli` (used as `cargo tauri`)
- Node (current LTS) and pnpm
- macOS, Windows or Linux for desktop. Android builds also need Homebrew OpenJDK 21 and the Android command-line tools.

## First-time setup

```sh
pnpm install
make setup      # one-time Android toolchain + signing (optional if you only target desktop)
```

`make setup` runs `scripts/setup-android.sh` (installs and prunes the NDK, removes the emulator, adds the arm64 Rust target, writes a git-ignored `.env.local`) and `scripts/setup-signing.sh` (creates signing keys under `~/.mnemora/`, with passwords in the macOS Keychain). It changes your system, so read both scripts first. Never re-run `cargo tauri android init`: `src-tauri/gen/android` is committed and customised (see [standards](docs/engineering/standards.md#android-project)).

## Daily commands

| Command | What it does |
|---|---|
| `make dev` | Vite plus a key menu; build the app on **Production** data (DB is backed up before migrations) |
| `make sandbox` | Vite plus a key menu; build the app on separate **Sandbox** data, with an orange SANDBOX badge |
| `make seed-sandbox` | Fill the **Sandbox** database with about three weeks of Journal Entries and Tasks, dated from today (quit the Sandbox app first) |
| `make build` | Release build for this machine (`.app` / `.dmg`) |
| `make android-dev` | `make sandbox` that also launches the Android app on a USB-connected phone (Sandbox data) |
| `make android-sandbox` | Debug Sandbox APK (no hot reload) installed on the phone |
| `make android-test` | Kotlin unit tests for the Todo widget |
| `make android` | Signed arm64 release APK, installed on the connected phone |
| `make test` | Rust tests plus Vitest |
| `make lint` | clippy (`-D warnings`), rustfmt check, Biome, svelte-check |
| `make fmt` | Format Rust and frontend code |
| `make clean-heavy` | Delete `target/`, `node_modules/` and Android build dirs (asks first) |
| `make bindings` | Regenerate `src/lib/ipc/bindings.ts` without launching the app |
| `make icons` | Regenerate every app icon and the favicon from `src-tauri/icons/source/*.svg` |

Every target runs with `CARGO_BUILD_JOBS=2` and cleans up the processes it started.

### Seeding the Sandbox

`make seed-sandbox` writes the recent-changes fixture into the Sandbox database, timed from now in this Mac's zone, so yesterday's changes can be read in the app and the change Tools. Quit the Sandbox app first. The seed refuses, before it writes anything, any folder inside Production's data (including the folder Production's device file moved it to), a database another connection is writing to, and a database holding rows that are not the fixture's. `--reset` deletes the Sandbox database files and seeds again; it never touches Production. Running it twice changes nothing.

```sh
make seed-sandbox ARGS="[--zone <IANA zone>] [--data-dir <folder>] [--base <folder>] [--reset]"
```

### Dev menu (`make dev`, `make sandbox`)

Both start only Vite on port 47821 and show a key menu. The native app is built only when you ask for it, and native builds run one at a time.

| Key | Action |
|---|---|
| `w` | Open the web mock in your browser |
| `z` | Launch the Mac app (Production for `make dev`, Sandbox for `make sandbox`) |
| `a` | Launch the Android app on a USB phone (`make sandbox` only; it always uses Sandbox data) |
| `r` | Relaunch the running or queued app(s) |
| `x` | Stop the native app(s), keep Vite; queued builds stay queued and say so |
| `?` | Show the menu again |
| `q` or Ctrl-C | Quit everything |

Flags launch apps as soon as Vite is up, and combine: `make dev ARGS="--mac --android"`. Without `make` you can run `pnpm dev:menu --mac`; the script refuses a taken port itself, but skips `.env.local`. Only one native build runs at a time. A build holds its slot until its build phase ends: the Mac build until cargo prints `Finished` or `Running`, and the Android build until Gradle prints `Installing APK`. If no such line appears within 30 minutes, the slot is freed anyway. A running app holds no slot. Without a terminal (`make dev </dev/null`) it runs Vite only and logs that keys are unavailable. Android needs exactly one phone connected, authorised for USB debugging; the menu says so when it is not.

## Trying the Todo widget

The Todo widget (Android 12+) is a native home-screen widget for the Todo Module. Try it on a USB-connected phone with USB debugging on:

1. `make android-sandbox` builds a debug **Sandbox** app and installs it (no hot reload). An installed Sandbox signed with the release key must be uninstalled first.
2. Open the Sandbox app once, so it creates its data, and add a few Tasks in Todo.
3. On the phone, long-press the home screen, choose Widgets, and drag **Todo (Sandbox)** from Mnemora onto a page. Setup opens: pick the view, theme and background, then Done. The gear in the Todo widget's header reopens setup.
4. Resize it: 3x2 and larger shows the list, smaller sizes become an "Add a task..." button, and 1x1 a plain +.

`make android-test` runs the Kotlin unit tests. The phone checklist is in [standards](docs/engineering/standards.md#todo-widget-phone).

## Environments

| | Production | Sandbox |
|---|---|---|
| Started by | `make dev` (press `z` or `ARGS="--mac"`), `make build` | `make sandbox` (press `z`), `make android-dev` |
| Identifier | `com.hemang.mnemora` | `com.hemang.mnemora.sandbox` |
| Data | `<platform data root>/com.hemang.mnemora/data/mnemora.db` | `<platform data root>/com.hemang.mnemora.sandbox/data/mnemora.db` |
| Config, logs | `.../config/device.toml`, `.../logs/` | same layout under the Sandbox folder |

The Environment is fixed at build time, so the two can never share a file. On macOS the platform data root is `~/Library/Application Support`.

## Project layout

```
crates/core/     Shared Rust: environments, paths, SQLite + migrations, Settings, events, logging, errors
src-tauri/       Thin Tauri app crate: commands, window state, theme injection, icons
src/lib/ui/      Every shared UI Component
src/lib/         theme, router wrapper, ipc (generated bindings), time, settings store
src/shell/       AppShell, Sidebar, TabBar, registry of Modules
src/modules/     One folder per Module (home, settings)
docs/            adr/ (decisions), engineering/ (standards), design/ (UI system + pitch)
scripts/         setup and signing helpers; dev.mjs and dev/ run the dev menu (rules in dev/, tests beside them)
```

## Docs

- [Coding standards](docs/engineering/standards.md), including how to add a Module
- [UI system](docs/design/ui-system.md) and the [visual reference](docs/design/ui-pitch.html)
- [Architecture decisions](docs/adr/)
- [Glossary](GLOSSARY.md)
