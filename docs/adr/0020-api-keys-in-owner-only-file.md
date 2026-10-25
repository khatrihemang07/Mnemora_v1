# API keys live in an owner-only file, not the OS keychain

Each Provider's key is written to a file that only the person's account can read, in the app's private directory, keyed by Provider id and kept separate per Environment. The key is write-only: the screen learns only whether one is set, and can set, replace or clear it. Keys are never synced, never logged, and excluded from Android backup. The trade-off is that the key is protected by file permissions rather than encryption, so anyone who can read the person's account can read it, which is the same owner-only approach Sphinx takes for its own config (written with mode 0600 and renamed into place), in exchange for one path that behaves the same on macOS and Android with no prompts, no platform plugin and no new dependency. The Android backup exclusion is not yet configured: the manifest has no backup rules today, so the keys ticket must add them.

## Considered Options

- **OS keychain (macOS Keychain or a keyring crate)**: rejected; it brings prompts on macOS and a new dependency, and on Android it needs a Keystore helper in Kotlin and JNI.
- **Android Keystore through the Tauri mobile plugin**: rejected for the same reasons, and it would make the desktop and phone paths differ.
- **Keys in the SQLite database**: rejected; the database is backed up and is the thing sync reads, so a key would travel with the data.
- **Keys in a synced Preference**: rejected; a key on one device would be sent to all of them.
- **Environment variables**: rejected; a GUI app cannot rely on them, and the person would have to set them outside the app.
