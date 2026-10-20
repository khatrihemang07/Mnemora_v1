# Rule tests at the command layer, mock kept honest by shared fixtures

Business rules are tested through the Tauri commands with `TestApp`, so a test covers validation, the service and the database as the UI sees them; crates keep unit tests only for pure functions. The browser mock re-implements some rules (search, edit grace, error messages), so those rules live in shared JSON fixtures that both a `TestApp` test and a Vitest test against the mock run. This keeps the mock from drifting without adding E2E tests (ADR 0006).
