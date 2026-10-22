# Sync-ready SQLite, one file per Environment

Data lives in one SQLite file per Environment (WAL, via bundled rusqlite), with each Module owning its migrations. Tables use UUIDv7 ids, `created_at`/`updated_at` and soft delete, so records can be merged by a future sync without a rewrite. There is no encryption yet; storage sits behind one boundary so SQLCipher can be added later.

## Amendment (2026-10-22): sync is real

The "future sync" is now built (ADR 0017). The choices above held up: UUIDv7 ids and soft delete are what let two devices create and delete records without clashing, and `updated_at` is bumped by every syncable mutation. Soft-deleted rows are no longer purged, since a deleted row is its own tombstone. Merging uses the change log (ADR 0008), not table diffs. Sync opens its own connection to the same file (WAL) with short transactions, so commands never wait behind a sync, and both databases are backed up before the first merge with each new device. Encryption is still not added; sync traffic is TLS between pinned keys.
