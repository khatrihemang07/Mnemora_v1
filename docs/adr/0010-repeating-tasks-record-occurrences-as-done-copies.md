# Repeating Tasks record occurrences as done copies

A repeating Task is one row that never closes. Completing it inserts a second row, the occurrence: a done copy of the Task as it was (title, List, priority, notes, and the Due date and time of that occurrence) whose `repeat_of` points at the live Task and whose `closed_at` is now. The live Task then moves to its next Due date, keeps its time and gets its Checklist items unticked. The Task's Comments move to the copy, because they belong to the occurrence they were written for. The change log gets a `create` for the copy and a `tick` for the Task.

Everything that already understands a done Task (the Done view, Today's "Completed today", search, Undo, sync) understands an occurrence with no special case; only the live Task is special. Reopening works on the latest occurrence only: it soft-deletes the copy, hands the Comments back, puts the Task on that occurrence's date and time and restores the Checklist ticks unless the notes were edited since. That is also Undo for the tick. An older occurrence cannot be reopened, since rewinding past a newer one would invent a history; the UI says so. Deleting a repeating Task soft-deletes only that row, so its occurrences stay in Done (reopening one then explains that its Task is gone). Skipping moves the Task on and records nothing.

The recurrence maths are pure functions in `crates/todo/src/recurrence.rs` (a TypeScript copy serves the When picker and the browser mock; both run `recurrence-rules.json`). A fixed Repeat counts from the Due date and goes to the first occurrence after today, which is one step after the Due date when completed early and skips missed dates when late; "after done" counts from the day it is done and is only offered for an interval, not chosen weekdays. A month or year Repeat keeps the day of the month of its Due date in `repeat_days` so a 31st clamped to the 28th comes back as the 31st; changing the date by hand makes the new date the anchor.

## Considered Options

- **Move the Task and keep only a counter or a log line**: rejected; Done would need a second source, and Comments and notes at that moment would be lost.
- **Make the next occurrence a new Task and close the old one**: rejected; the Task would change identity every time, breaking links, History and the sort order.
- **Generate future occurrences ahead of time**: rejected; projections are not Tasks, they pile up when ignored, and editing the rule would mean rewriting them.
