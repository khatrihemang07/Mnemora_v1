//! Todo's rows that the change log does not (fully) describe, brought into it before Sync reads
//! it (`Module::backfill_log`). Task, List and Comment text must never reach a log line.

use mnemora_core::changelog::{self, Change, ChangeOp, Unlogged};
use mnemora_core::error::AppResult;
use rusqlite::Connection;

use crate::{MODULE_ID, comment_repo, comment_service, list_repo, list_service, repo, service};
use crate::{comment_sync, list_sync, sync};

/// Logs every List, Task and Comment (deleted ones included) whose state the log lacks, Lists
/// first, then Tasks, then Comments, so that what a row refers to is met first. Tasks that
/// repeat come before their occurrences (the order `repo::every` gives).
pub(crate) fn run(conn: &Connection) -> AppResult<()> {
    let mut lists = Vec::new();
    for (list, deleted_at) in list_repo::every(conn)? {
        lists.push(Unlogged {
            change: change(
                list_service::ENTITY,
                &list.id,
                list_service::snapshot_of(&list, deleted_at)?,
            ),
            ms: list.updated_at,
            after: Vec::new(),
            needs_create: true,
        });
    }
    let mut written = changelog::backfill(conn, &list_sync::LIST, lists)?;

    let tasks_in_order = repo::every(conn)?;
    let mut tasks = Vec::new();
    for (task, deleted_at) in &tasks_in_order {
        let list = task.list_id.as_deref().map(|l| (list_service::ENTITY, l));
        tasks.push(Unlogged {
            change: change(
                service::ENTITY,
                &task.id,
                service::snapshot_of(task, *deleted_at, false)?,
            ),
            ms: task.updated_at,
            after: list.into_iter().collect(),
            needs_create: true,
        });
    }
    written += changelog::backfill(conn, &sync::TASK, tasks)?;

    let comments = comment_repo::every(conn)?;
    let mut rows = Vec::new();
    for (comment, deleted_at) in &comments {
        rows.push(Unlogged {
            change: change(
                comment_service::ENTITY,
                &comment.id,
                comment_service::snapshot_of(comment, *deleted_at)?,
            ),
            ms: comment.updated_at,
            after: vec![(service::ENTITY, comment.task_id.as_str())],
            needs_create: true,
        });
    }
    written += changelog::backfill(conn, &comment_sync::COMMENT, rows)?;
    if written > 0 {
        tracing::info!(target: "sync", records = written, "Todo rows missing from the change log were added");
    }
    Ok(())
}

fn change(entity: &str, id: &str, snapshot: serde_json::Value) -> Change {
    Change {
        module: MODULE_ID.into(),
        entity: entity.into(),
        entity_id: id.into(),
        op: ChangeOp::Update,
        session_id: None,
        snapshot,
    }
}
