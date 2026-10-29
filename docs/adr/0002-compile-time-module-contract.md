# Modules use a compile-time contract and never import each other

Every Module implements the Rust `Module` trait (id, migrations, Settings schema) and ships a UI manifest, and is registered in exactly one place per side (`modules()` in the app crate, `src/shell/registry.ts`). Modules never import each other; they share only core contracts (the `Module` trait, Settings, and later `EntityRef` and `SearchProvider`) and the typed `EventBus`. A compile-time contract gives type-checked boundaries and a small binary, and keeps Modules independently removable.

## Considered Options

- **Runtime plugins**: rejected; they add loading, sandboxing and versioning problems we do not need for a single-developer app.

## Amendment (2026-10-22): Modules can be off by default, and declare what syncs

The `Module` trait and the UI manifest gain `default_enabled`. A Module that is off by default (Sync) is enabled only when the user opts in: a core Preference lists the opted-in default-off Modules, so a Module is enabled when it is on by default and not disabled, or off by default and opted in. Sync's own on/off is the exception, a Device setting that is never synced, because each device chooses whether it talks to the network.

The trait also gains a sync contract (ADR 0017). A Module declares its syncable entities and classifies each field as text (three-way merge), value (newest wins), order (newest wins, reported in one line) or derived (recomputed locally, never synced). It applies a set of remote field values through its own repository and validation, as ADR 0001 requires, records the result as a remote change rather than a new local one and emits its usual change events (`apply_remote`). After a remote record is stored, `settle_remote` lets the Module finish what that record implies (for example merging two Lists with one name), recording its own changes as ordinary ones. Text is not merged by the Module: the engine replays the records (ADR 0008). The Sync Module drives this contract and still imports neither Journal nor Todo.

The trait also gains `holdings` (default: nothing): the Entries, Tasks and List names a Module holds, deleted ones excluded, which Sync shows in the preview before two devices that both have data merge for the first time. Core and Settings hold nothing a person counts.

## Amendment (2026-10-25): Modules offer AI Tools through the contract

The `Module` trait gains an optional method, which defaults to offering no Tools. A Module that wants the AI to read its data returns read-only Tools (ADR 0018), each described by a small Tool contract that lives in the shared core. The AI Module collects the Tools of every Module that is turned on and offers only those; a Module that is off offers none, and the AI's prompt says which Tools are unavailable. Journal and Todo therefore depend only on core, never on the AI Module, and a future Module adds Tools by implementing the method, without any change to the AI Module. Tools never write: the contract has no write path, so the AI cannot change a Module's data.

Rejected for the AI Tools: the AI Module importing Journal and Todo and calling their services (it breaks the rule that Modules never import each other, and every new Module would need an edit to the AI Module); a Tool registry inside the AI Module filled in by hand (the same edit is needed for every new Module); and runtime plugins that register Tools (rejected above for Modules).

## Amendment (2026-10-29): the Tool calendar carries the client's time zone

The `Calendar` a Tool receives gains the IANA time zone the client sends with each question, next to today's date. Tools turn stored instants into local dates and times and into day ranges in that zone (core's `local_time`), never in the machine's own zone, so the same question gives the same answer on any device. A Tool that gets a zone it cannot read refuses the question with an error the Model can see. This is why core now depends on `jiff` with its bundled time-zone data.
