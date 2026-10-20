# App-wide Change Log in Core

Core keeps one `change_log` table that any Module writes to when an entity changes. Each record holds the full JSON snapshot of the entity after the change, with its Module, entity kind, entity id, operation and timestamp. Changes made within one editing session carry a session id and replace that session's single record, so many autosaves become one record. Records for live entities are kept forever and purged together with an entity when it is deleted for good (the Journal purges 30 days after deletion). The log is internal in v1: nothing reads it over IPC yet. It is the basis for sync and for a later history view.

## Considered Options

- **Diffs**: rejected; they are small but cannot be read, restored or merged without replaying the chain, and one lost record corrupts everything after it.
- **Metadata only** (who, what, when): rejected; it cannot restore content or feed a sync.
- **Per-Module history tables**: rejected; every Module would reimplement session batching and purging, and sync would have to know each table. One core table keeps that in one place.
