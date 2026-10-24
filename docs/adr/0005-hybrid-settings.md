# Hybrid Settings: SQLite Preferences plus `device.toml`

Preferences (theme, formats, enabled Modules) are stored in the database `settings` table with a typed Rust schema per Module, so they sync with the data. Device settings (data folder, window state, sidebar width, log level) live in `device.toml` in the config folder, because they are per device, never synced, and some are needed before the database opens. The theme is injected into the window before first paint so there is no flash.

## Considered Options

- **Everything in SQLite**: rejected; the data path and log level must be known before the database is opened.
- **Everything in a file**: rejected; Preferences should travel with the data.

## Amendment (2026-10-22): look and feel is per device

Theme, density, text size, reduce motion and the phone tabs (with the flag that records they were edited) move from Preferences to `device.toml`, so a phone and a Mac can look different once they sync. A one-time upgrade copies the existing values into the device file and deletes the rows. Theme injection before first paint reads the device file, which also keeps it independent of the database. The other Preferences (formats, week start, name, enabled Modules, Journal and Todo view choices) still sync, and every Preference write is recorded in the change log. `device.toml` also holds the device's identity (a random UUIDv7 per install and Environment, created on first run) and whether Sync is on.
