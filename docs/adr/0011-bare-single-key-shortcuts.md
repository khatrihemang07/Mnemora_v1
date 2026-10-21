# Bare single-key shortcuts work app-wide, but only when nothing is being typed

Some actions are worth one key: **Q** adds a Todo Task from anywhere, and **N** will start a Journal Entry from anywhere. A Module claims such a key in its manifest (`bareKeys`), and the shell listens once for the whole app. The key fires only when all of these hold: no modifier is held (a bare key, not ⌘Q or ⇧Q), the target is not a text field (`input`, `textarea`, `select`, a contenteditable or a textbox), no dialog, alert dialog, menu or listbox is open, the key is not a held-down repeat or part of an IME composition, and nothing has already handled it. The rule is one pure function (`bareKeyFires` in `src/lib/bare-key.ts`) with its own tests, so N reuses it instead of growing a second copy. If two Modules claim one key, the first enabled Module by navigation order wins; a Module that is switched off gives its keys up.

Q is Todo's and N is the Journal's, and neither Module uses the other's key. ⌘N stays the Journal's "new" shortcut (`create` in the manifest); it is a modified key and never conflicts with a bare one. Inside Todo, Q opens the capture box in place on the page that is showing (the first "+ New task" row, so the Task inherits Today, a List or an Upcoming day); anywhere else, and on a Todo page with no such row, it opens the floating Quick add, which adds to the Inbox. The floating box is mounted by the shell for as long as the Module is on (`overlay` in the manifest), so no Module imports another or the shell.

## Considered Options

- **A modifier on every shortcut (⌘Q, ⌘⇧N)**: rejected; ⌘Q quits the app on macOS and the point of Q is one-handed speed, the way Things and Todoist work.
- **Only inside the Module's own pages**: rejected; "add a Task" is most useful when you are somewhere else, such as writing in the Journal.
- **A global capture shortcut owned by the shell**: rejected; the shell would have to know what a Task is. A key a Module claims keeps the shell Module-agnostic (ADR 0002).
- **Fire inside text fields when the field is empty**: rejected; a letter that sometimes types and sometimes opens a box is the kind of surprise this app avoids.
