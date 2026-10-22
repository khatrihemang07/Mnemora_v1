# Todo widget calls Rust over JNI

The Todo widget is native Android code (RemoteViews), but it never has rules of its own. Every read, tick, undo, parse and add goes through one JSON-in, JSON-out function exported by the app's Rust library over JNI, which calls the same rule functions as the Tauri commands. A tick from the widget therefore writes the same change log, History, Repeat occurrences and Open steps as a tick in the app (ADR 0001), and every user-facing string in a snapshot is final text built in Rust, so Kotlin does no date or wording logic.

Loading the library from a broadcast receiver does not start Tauri. When the app process is alive the bridge uses the app's own connection and emits a typed `TodoChangedEvent`, so a running app updates at once. Otherwise it opens a cached connection to the database that app start-up resolves (same Environment guard, paths and data-dir override, from the Android data dir). That connection is opened only if the file exists and is never migrated: a missing database is reported as `not_set_up` and a pending migration as `needs_update`, and only the app creates and upgrades data. Logging initialisation is idempotent, because the widget may start the process before Tauri does.

Release builds abort on panic, so nothing on the JNI path may unwrap, expect or index unchecked; every failure becomes an `err` reply.

Writes in the app notify the widget through one wrapper around the Todo write commands, which broadcasts a package-scoped intent on Android (so Production and Sandbox never cross). Tests count the signals instead.

## Considered Options

- **Re-implement the rules in Kotlin**: rejected; two copies of Repeats, History and Open steps would drift (ADR 0001).
- **Read the database from Kotlin, write through the app**: rejected; the widget must also tick and add while the app is closed, and a second SQL layer is a second set of rules.
- **Start Tauri headless from the widget**: rejected; far too heavy for a broadcast receiver on an 8 GB-class budget.
- **Let the widget run migrations**: rejected; a receiver racing the app's start-up migration, with no backup step, risks the user's data.
