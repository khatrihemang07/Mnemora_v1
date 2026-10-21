# Manual order is a fractional-index key, one per Task and one per List

Every Task has one `sort_key` shared by every view (Today, a List, the Inbox, All), and every List has one in the List order. A key is a string of base-62 digits read as a fraction between 0 and 1, so it sorts bytewise as text in SQLite (`ORDER BY sort_key`) and there is always a key strictly between any two others. Moving a Task between two neighbours writes the Task's own row and nothing else; the neighbours are never renumbered. That keeps a move to one row and one change-log record, and it merges cleanly in a future sync, where two devices moving different Tasks touch different rows.

The keys are made by a pure module, `crates/todo/src/order.rs` (a TypeScript copy serves the browser mock; both run the same cases). Appends step the first digit that can still grow (`V`, `W`, `X`, … `z`, `zV`, …) so a long list stays short; inserting between two neighbours takes the midpoint, which adds roughly a digit per six inserts into the same gap. A canonical key never ends in `0`, because trailing zeros add nothing to a fraction and leave no room after it: the keys of the first Todo slices (`V0000000`, counting up) had them, so a migration trims the zeros (the order of equal-length digit strings is the same either way) and the code reads any key without them.

Views can disagree with the manual order (a sort, or Priority inside a time block), so `todo_task_reorder` takes the two neighbours the user saw, and when they are out of order or not adjacent in the whole order it puts the new key just after the upper one or just before the lower one. Dropping a Task into a group also adopts that group's property in the same write: a day sets the Due date (never a past one: Overdue adopts nothing, and an explicit past day is refused), a Priority sets the Priority, a List moves it. One change is recorded, `move` when the List changed and `update` otherwise.

Sorts never touch the keys. A List's sort and grouping live on its row, the smart views' grouping and the Lists' sort are Preferences, and "Manual" simply reads the keys again, so the drag order survives any amount of sorting.

## Considered Options

- **Integer positions, renumbered on a move**: rejected; a move rewrites many rows and conflicts in sync.
- **Floating-point positions**: rejected; they run out of precision after a few dozen inserts in one gap, and there is no cheap way to tell.
- **A second order for Today (as Things does)**: rejected; one shared order means reordering anywhere is consistent, and Today's own rule (timed first, then Priority inside a time block) is a sort, not a second order.
