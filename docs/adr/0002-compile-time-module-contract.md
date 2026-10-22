# Modules use a compile-time contract and never import each other

Every Module implements the Rust `Module` trait (id, migrations, Settings schema) and ships a UI manifest, and is registered in exactly one place per side (`modules()` in the app crate, `src/shell/registry.ts`). Modules never import each other; they share only core contracts (the `Module` trait, Settings, and later `EntityRef` and `SearchProvider`) and the typed `EventBus`. A compile-time contract gives type-checked boundaries and a small binary, and keeps Modules independently removable.

## Considered Options

- **Runtime plugins**: rejected; they add loading, sandboxing and versioning problems we do not need for a single-developer app.

## Amendment (2026-10-22): Modules can be off by default, and declare what syncs

The `Module` trait and the UI manifest gain `default_enabled`. A Module that is off by default (Sync) is enabled only when the user opts in: a core Preference lists the opted-in default-off Modules, so a Module is enabled when it is on by default and not disabled, or off by default and opted in. Sync's own on/off is the exception, a Device setting that is never synced, because each device chooses whether it talks to the network.

The trait also gains a sync contract (ADR 0017). A Module declares its syncable entities and classifies each field as text (three-way merge), value (newest wins), order (newest wins, reported in one line) or derived (recomputed locally, never synced). It applies a set of remote field values through its own repository and validation, as ADR 0001 requires, records the result as a remote change rather than a new local one and emits its usual change events. The Sync Module drives this contract and still imports neither Journal nor Todo.
