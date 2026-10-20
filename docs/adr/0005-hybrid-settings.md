# Hybrid Settings: SQLite Preferences plus `device.toml`

Preferences (theme, formats, enabled Modules) are stored in the database `settings` table with a typed Rust schema per Module, so they sync with the data. Device settings (data folder, window state, sidebar width, log level) live in `device.toml` in the config folder, because they are per device, never synced, and some are needed before the database opens. The theme is injected into the window before first paint so there is no flash.

## Considered Options

- **Everything in SQLite**: rejected; the data path and log level must be known before the database is opened.
- **Everything in a file**: rejected; Preferences should travel with the data.
