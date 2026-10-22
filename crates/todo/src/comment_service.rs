//! Comments: rules, persistence and change-log records in one transaction. Comment text must
//! never reach a log line.

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::error::AppResult;
use mnemora_core::util::{new_id, now_ms};
use rusqlite::Connection;

use crate::MODULE_ID;
use crate::comment::{Comment, validate_body};
use crate::error::TodoError;
use crate::{comment_repo, repo};

/// Change-log entity name for Comments.
pub(crate) const ENTITY: &str = "todo_comment";

pub(crate) fn record(
    conn: &Connection,
    c: &Comment,
    op: ChangeOp,
    deleted_at: Option<i64>,
) -> AppResult<()> {
    let mut snapshot = serde_json::to_value(c).map_err(TodoError::Snapshot)?;
    if let Some(fields) = snapshot.as_object_mut() {
        fields.insert("deleted_at".into(), deleted_at.into());
    }
    changelog::record(
        conn,
        &Change {
            module: MODULE_ID.into(),
            entity: ENTITY.into(),
            entity_id: c.id.clone(),
            op,
            session_id: None,
            snapshot,
        },
    )
}

/// A Task's Comments, oldest first.
pub fn comments(conn: &Connection, task_id: &str) -> AppResult<Vec<Comment>> {
    repo::get(conn, task_id)?.ok_or(TodoError::TaskNotFound)?;
    comment_repo::for_task(conn, task_id)
}

/// Adds a Comment to a Task, whatever state the Task is in.
pub fn add_comment(conn: &mut Connection, task_id: &str, body: &str) -> AppResult<Comment> {
    let body = validate_body(body)?;
    let tx = conn.transaction()?;
    repo::get(&tx, task_id)?.ok_or(TodoError::TaskNotFound)?;
    let now = now_ms();
    let comment = Comment {
        id: new_id(),
        task_id: task_id.to_owned(),
        body: body.to_owned(),
        edited: false,
        created_at: now,
        updated_at: now,
    };
    comment_repo::insert(&tx, &comment)?;
    record(&tx, &comment, ChangeOp::Create, None)?;
    tx.commit()?;
    Ok(comment)
}

/// Replaces a Comment's text and marks it edited. The same text changes nothing.
pub fn edit_comment(conn: &mut Connection, id: &str, body: &str) -> AppResult<Comment> {
    let body = validate_body(body)?;
    let tx = conn.transaction()?;
    let mut comment = comment_repo::get(&tx, id)?.ok_or(TodoError::CommentNotFound)?;
    if comment.body == body {
        return Ok(comment);
    }
    body.clone_into(&mut comment.body);
    comment.updated_at = now_ms().max(comment.updated_at + 1);
    comment.edited = true;
    comment_repo::update_body(&tx, &comment)?;
    record(&tx, &comment, ChangeOp::Update, None)?;
    tx.commit()?;
    Ok(comment)
}

/// Soft-deletes a Comment; `restore_comment` is Undo. The row stays for good: it is its own
/// tombstone.
pub fn delete_comment(conn: &mut Connection, id: &str) -> AppResult<()> {
    let tx = conn.transaction()?;
    let mut comment = comment_repo::get(&tx, id)?.ok_or(TodoError::CommentNotFound)?;
    let now = now_ms().max(comment.updated_at + 1);
    comment_repo::set_deleted(&tx, id, Some(now), now)?;
    comment.updated_at = now;
    record(&tx, &comment, ChangeOp::Delete, Some(now))?;
    tx.commit()?;
    Ok(())
}

/// Brings a deleted Comment back, where it was.
pub fn restore_comment(conn: &mut Connection, id: &str) -> AppResult<Comment> {
    let tx = conn.transaction()?;
    let (mut comment, deleted_at) =
        comment_repo::get_any(&tx, id)?.ok_or(TodoError::CommentNotFound)?;
    let Some(deleted_at) = deleted_at else {
        return Err(TodoError::Validation("That comment isn't deleted").into());
    };
    let now = now_ms().max(deleted_at + 1);
    comment_repo::set_deleted(&tx, id, None, now)?;
    comment.updated_at = now;
    record(&tx, &comment, ChangeOp::Restore, None)?;
    tx.commit()?;
    Ok(comment)
}
