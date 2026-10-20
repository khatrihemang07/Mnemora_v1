# Sync-ready SQLite, one file per Environment

Data lives in one SQLite file per Environment (WAL, via bundled rusqlite), with each Module owning its migrations. Tables use UUIDv7 ids, `created_at`/`updated_at` and soft delete, so records can be merged by a future sync without a rewrite. There is no encryption yet; storage sits behind one boundary so SQLCipher can be added later.
