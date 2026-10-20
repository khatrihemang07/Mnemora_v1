# Modules use a compile-time contract and never import each other

Every Module implements the Rust `Module` trait (id, migrations, Settings schema) and ships a UI manifest, and is registered in exactly one place per side (`modules()` in the app crate, `src/shell/registry.ts`). Modules never import each other; they share only core contracts (the `Module` trait, Settings, and later `EntityRef` and `SearchProvider`) and the typed `EventBus`. A compile-time contract gives type-checked boundaries and a small binary, and keeps Modules independently removable.

## Considered Options

- **Runtime plugins**: rejected; they add loading, sandboxing and versioning problems we do not need for a single-developer app.
