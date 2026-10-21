# App-wide Change Log in Core

Core keeps one `change_log` table that any Module writes to when an entity changes. Each record holds the full JSON snapshot of the entity after the change, with its Module, entity kind, entity id, operation and timestamp. Changes made within one editing session carry a session id and replace that session's single record, so many autosaves become one record; a session that created the entity stays recorded as a `create`. Operations are `create`, `update`, `move`, `tick`, `delete` and `restore`. Records for live entities are kept forever and purged together with an entity when it is deleted for good (the Journal purges 30 days after deletion). The raw log never crosses IPC, but a Module may read its own records to build a view: Todo's Task History (see the amendment below). It is also the basis for sync.

## Considered Options

- **Diffs**: rejected; they are small but cannot be read, restored or merged without replaying the chain, and one lost record corrupts everything after it.
- **Metadata only** (who, what, when): rejected; it cannot restore content or feed a sync.
- **Per-Module history tables**: rejected; every Module would reimplement session batching and purging, and sync would have to know each table. One core table keeps that in one place.

## Amendment: the change log is readable over IPC through History

The Task sheet's History fold is built from this log, so it is no longer internal. Core gains `changelog::read(entity, entity_id)`, which returns one entity's records oldest first. The Todo crate turns consecutive snapshots into readable lines in Rust (`crates/todo/src/history.rs`, command `todo_task_history`): "Due date Thu 22 Oct → Fri 23 Oct", "Priority P4 → P1", "Moved 📥 Inbox → 💼 Work", "Done (occurrence of 18 Oct)", "Created", "Title edited". Only these lines cross IPC, never snapshots, and the lines carry dates, list names and Priorities but never the Task's own text.

Batching is what the editing session was for: the Task sheet sends one session id per opening, so every autosave of the title and notes replaces one record. A session's record carries the latest snapshot and moves to the end of the log whenever it is rewritten, so other changes made during the opening (a Priority, a date) can sit before it and already contain some of its text. History therefore never compares text with the neighbouring record: it keeps a baseline of the title and notes as they were settled before the session (at creation, after an earlier session, or after a Checklist reset on completion) and compares the session's final text with that. One opening is exactly one "Title edited" and one "Notes edited" line, however much else happened in between. Task snapshots leave out derived fields (the Comment count). The Todo Module purges deleted Tasks, Lists and Comments after 30 days at startup, together with their records, as the Journal does.
