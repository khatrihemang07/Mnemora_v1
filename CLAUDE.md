## Agent skills

### Issue tracker

Issues are tracked in GitHub Issues for this repo (via the `gh` CLI). See `docs/agents/issue-tracker.md`.

### Triage labels

Default five-label vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `GLOSSARY.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.

## Coding standards

Follow [docs/engineering/standards.md](docs/engineering/standards.md) on every change, without being asked. It covers structure, naming, errors, logging, testing, performance and how to add a Module.

## UI system

All UI decisions and the Component inventory are in [docs/design/ui-system.md](docs/design/ui-system.md); the visual reference is `docs/design/ui-pitch.html`. Use tokens and `src/lib/ui` Components only, and never import `bits-ui` outside `src/lib/ui`.

## Resource constraints

The dev machine is an 8 GB Mac with limited disk.

- No language servers: no Serena tools, no rust-analyzer. Use Read, Grep, Edit and Bash.
- Prefix every cargo command with `CARGO_BUILD_JOBS=2`, run one build at a time, and prefer `cargo check`.
- Never run `cargo tauri dev/build` or Android commands unless asked. At most one `pnpm dev`.
- No browsers or emulators. Kill every process you start (`pgrep -fl vite`), and leave other projects' processes alone.
- No dependency changes without a stated need; pin exact versions.

## Key commands

`make dev` | `make sandbox` | `make build` | `make test` | `make lint` | `make fmt` | `make bindings`. See `README.md` for the full table.
