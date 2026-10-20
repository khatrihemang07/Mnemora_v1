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
| `make dev` | Hot-reload app on **Production** data (DB is backed up before migrations) |
| `make sandbox` | Hot-reload app on separate **Sandbox** data, with an orange SANDBOX badge |
| `make build` | Release build for this machine (`.app` / `.dmg`) |
| `make android-dev` | Hot reload on a USB-connected phone (Sandbox data) |
| `make android` | Signed arm64 release APK, installed on the connected phone |
| `make test` | Rust tests plus Vitest |
| `make lint` | clippy (`-D warnings`), rustfmt check, Biome, svelte-check |
| `make fmt` | Format Rust and frontend code |
| `make clean-heavy` | Delete `target/`, `node_modules/` and Android build dirs (asks first) |
| `make bindings` | Regenerate `src/lib/ipc/bindings.ts` without launching the app |

Every target runs with `CARGO_BUILD_JOBS=2` and cleans up the processes it started.

## Environments

| | Production | Sandbox |
|---|---|---|
| Started by | `make dev`, `make build` | `make sandbox`, `make android-dev` |
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
scripts/         setup and signing helpers
```

## Docs

- [Coding standards](docs/engineering/standards.md), including how to add a Module
- [UI system](docs/design/ui-system.md) and the [visual reference](docs/design/ui-pitch.html)
- [Architecture decisions](docs/adr/)
- [Glossary](GLOSSARY.md)
