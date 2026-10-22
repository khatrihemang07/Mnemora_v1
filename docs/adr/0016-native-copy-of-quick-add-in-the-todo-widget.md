# Native copy of Quick add in the Todo widget

The Todo widget's + opens Quick add over the home screen without starting the app. A web view cannot do that cheaply: the Tauri activity takes seconds to cold-start and would show the whole app, and a receiver cannot host one. So the sheet is a second, native copy of the app's Quick add (`QuickAddActivity`, a translucent bottom sheet in the widget's own theme, with its own task, kept out of Recents). That makes two Quick adds, and the rule of this ADR is that only the part with no rules in it is written twice.

**What exists once.** Everything that decides what a Task becomes stays in Rust (ADR 0001, 0014). Reading the typed words is the app's own parser (`parse_text`), called through the bridge's `parse` method, so a recognised word, its meaning ("Fri 23 Oct", "9:30 PM", "Every 2 weeks") and its offsets are the same text the app would show. Adding is the bridge's `add`, a Rust port of the Capture box's `resolveFields` and its record of understood words that goes through the same `create_task` as the app's command, creating a List for an unknown `#Name`. The toast wording (`Added “Call plumber” to Today`) is built in Rust from ports of `landedIn` and `quickAddedMessage`. `capture_context` hands Kotlin the placeholder, the place the sheet opened in (Today: due today, Upcoming: tomorrow, a List: that List, Inbox and All: the Inbox, as the app's phone + does), the Lists and the Settings.

**What exists twice.** The keyboard-side helpers that need no database: words kept as text, read again, "use instead", pruning, the word ⌫ keeps as text, the `#` query and its List suggestions, the marked stretches of the title and the multi-line paste split. They are `capture.ts` and `CaptureRules.kt`. The sheet's views, chips and pickers are Android views, not Svelte Components; they follow `docs/design/ui-system.md` by sharing the same tokens through the widget's theme resources, not by sharing code. The sheet has no When picker (calendar, time, Repeat): the When chip only shows what the words and the place give the Task.

**Known differences from the app.** (1) The reading chips keep showing the previous reading for the few milliseconds until the new reading arrives; the app hides them while the reading is stale. This is deliberate, because it flickers less.

**How parity is kept.** Shared JSON fixtures, run on both sides, are the contract:

- `capture-ui.json`: the helpers above. Vitest runs it against `capture.ts` (`capture-ui.test.ts`), JUnit against `CaptureRules.kt` (`CaptureRulesTest`).
- `capture-resolve.json`: how typed words, chips and the place settle the Task's fields. Vitest runs it against `resolveFields`, Rust against `resolve_fields`.
- `widget-text.json`: the toast, chip meanings and the When chip label, run by Vitest against `tasks.ts`, `dates.ts` and `understood.ts`, and by Rust against `widget_text.rs`.

The bridge itself is tested at its JSON entry point against a temporary database (ADR 0009): inheritance for every view, `#` creating a List, Priority, notes, change-log rows, and that a Task added from the widget is the same row, change log and History as one created through the app's `todo_task_create`. A change to the Capture box's rules is therefore a change to a fixture, and it fails whichever side was forgotten.

## Considered Options

- **Open the app on its own Quick add**: rejected; it is the app starting, which is exactly the trip the widget exists to avoid.
- **Host the real Quick add in a web view over home**: rejected; the cost of starting Tauri from a widget tap (seconds, hundreds of MB) is what ADR 0014 refused for the receiver, and the same reason applies here.
- **A cut-down native field with no highlight or chips**: rejected in the design round; typing "tomorrow 9am p2" without seeing it understood makes the sheet something to double-check, so it would still be a second Quick add, only a worse one.
- **Port the parser to Kotlin**: rejected; the parser is the largest and most changeable rule set in the Capture box. It stays in Rust and is called, not copied.
- **Compose or a shared cross-platform UI layer**: rejected; a large dependency on an 8 GB machine, and the sheet is one screen.
