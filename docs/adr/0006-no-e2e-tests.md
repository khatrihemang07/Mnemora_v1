# No end-to-end tests for now

We rely on Rust unit and in-memory SQLite tests, Vitest for UI logic, and a manual smoke checklist. Desktop webview and Android E2E tooling is heavy and flaky for a small app on a memory-constrained dev machine, and the thin-UI design (ADR 0001) keeps most behaviour testable below the UI. Revisit when a Module has flows that unit tests cannot cover.
