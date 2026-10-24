# Mnemora

Mnemora is an offline-first life-management app made of independent Modules that share one shell, one look and one set of Settings.

## Language

**Module**:
A self-contained area of the app with its own data, screens and Settings, such as Journal, Todo or AI.
_Avoid_: Component, feature, space, app

**Component**:
A reusable UI building block (Button, Card, Dialog) shared by every Module.
_Avoid_: Widget, element, module

**Settings**:
User preferences: core ones that apply app-wide (theme, shortcuts, data) plus each Module's own section.
_Avoid_: Config, options

**Production**:
The Environment holding the user's real data.
_Avoid_: Live, prod data, real mode

**Sandbox**:
The Environment with separate, disposable data used for trying things out, never touching Production data.
_Avoid_: Test mode, staging, playground

**Environment**:
Which data set the app is running against: Production or Sandbox.
_Avoid_: Profile, mode, stage

**Preference**:
A user setting stored with the data, so it follows the data when synced, such as time format or week start.
_Avoid_: Option, config

**Device setting**:
A setting that belongs to one device and is never synced, such as window size, the data folder or the theme (look and feel is per device).
_Avoid_: Local preference, machine config

**Home**:
The start screen: a greeting, the current time and a place for Modules to surface what matters.
_Avoid_: Dashboard, landing page

### Journal

**Entry**:
One untitled piece of Journal writing with its own identity. It sits on exactly one Page and keeps the moment (date and time) it was written, which may fall on a different date than its Page. Moving it to another Page never changes that moment.
_Avoid_: Note, post, log, record

**Page**:
All the Entries for one calendar date, like a dated page in a paper diary. Today's Page is the default, but an Entry can be written on or moved to any other Page, past or future.
_Avoid_: Day, day page, date

**Editing session**:
One continuous stretch of writing or editing an Entry, from opening it until you finish. An Entry counts as edited only when it changes more than 10 minutes after it was written.
_Avoid_: Draft, revision


### Todo

**Task**:
One thing to do in the Todo Module, with its own identity. It may carry a Due date, a Repeat and a Priority, and is open, done or Won't do.
_Avoid_: Todo, to-do, item, reminder

**List**:
A named collection of Tasks with an optional emoji, such as Home or Groceries. A Task belongs to at most one List, and Lists do not nest. Names are unique ignoring case and accents, and at most 60 characters.
_Avoid_: Project, folder, area, category

**Inbox**:
Where Tasks with no List wait to be sorted. It also shows undated Tasks from Lists in a separate group, which does not count toward emptying it.
_Avoid_: Unsorted, default list

**Priority**:
How urgent a Task is, from P1 (most urgent) to P4. P4 means no priority and is the default. P1 to P3 show as a red, amber or violet ring on the checkbox, and inside a time block Today and Upcoming list the more urgent first.
_Avoid_: Importance, flag, level

**Won't do**:
A Task closed without being done. It is kept with the done Tasks, shown with a ⊗, but never counts as done. On a repeating Task it skips to the next Due date and keeps no record.
_Avoid_: Cancelled, dismissed, archived

**Comment**:
A short, dated remark added to a Task, kept apart from its notes. On a repeating Task, comments belong to the current occurrence and stay with that occurrence once it is done.
_Avoid_: Note, reply, message

**History**:
The list of every change made to a Task, oldest to newest, read from the app's record of changes. It cannot be edited. A Skip reads "Skipped (Thu → Sat)"; completing a repeating Task reads "Done (occurrence of 18 Oct)".
_Avoid_: Log, audit trail, activity

**Task sheet**:
The centred sheet (a bottom sheet on a phone) that shows one Task: title, List, When and Priority chips, notes, Comments, History and when it was created and done. It has its own address, `/todo?task=‹id›`, and saves every change as you make it.
_Avoid_: Detail view, editor, modal

**Open steps**:
The unticked Checklist items in a Task's notes. Completing such a Task asks "Cancel · Tick all and complete · Complete" every time.
_Avoid_: Subtasks, pending items

**Checklist item**:
A `- [ ]` line inside Markdown text (an Entry, or a Task's notes) that can be ticked in place. It is not a Task and has no date.
_Avoid_: Task, subtask, todo

**Due date**:
The one calendar date a Task is meant to be done on, optionally with a time of day. A Task without one is undated.
_Avoid_: Deadline, when, scheduled date

**Overdue**:
An open Task whose Due date is before today. A Task due today is never Overdue, even when its time has passed: it stays in Today with the time shown in red.
_Avoid_: Late, missed

**When picker**:
The compact picker that sets a Task's Due date, time and Repeat: a typed field, quick days, a rolling five-week calendar from this week (days a Repeat lands on are ringed), and a Time and a Repeat chip whose panels float over the calendar. Past days cannot be picked.
_Avoid_: Date picker (that is the Journal's), scheduler

**Capture box**:
The one box that adds or edits a Task in place: "+ New task", Q, the phone sheet and Quick edit all use it. As you type it recognises dates, weekdays, times, Repeats, `#List` and `p1`–`p4`, highlights those words and says how each was read ("tomorrow → Thu 22 Oct"). The words stay in the title. Property chips (List, When, Priority, Notes) set the rest by hand.
_Avoid_: Composer (that is the Journal's), add form, quick entry

**Recognised word**:
A word or phrase in a Task's title that capture read as a date, time, Repeat, List or Priority. It is highlighted while typing, and shown with a dotted underline and "Understood as …" afterwards, until the field it set is changed by hand. When two recognised words set the same kind of field, the last one wins; the others offer "use instead". Numeric dates ("2/3"), bare numbers and text in quotes or backticks are never recognised.
_Avoid_: Token, entity, tag

**Kept as text**:
A recognised word the user turned back into plain text (× on its chip, ⌫ right after it, or quotes). A "kept as text ↺" chip reads it again.
_Avoid_: Ignored, escaped, dismissed

**Quick add**:
Adding a Task with the Q key. Inside Todo it opens the capture box in place on the page showing (so the Task inherits Today, a List or an Upcoming day); anywhere else it opens a floating box that adds to the Inbox and says where the Task went. The Add button on Todo's Home card opens the same box with the Task due today.
On a phone the teal + button opens it for the view on show, and long-pressing the Todo tab opens it from any page.
_Avoid_: Quick capture, global add

**Quick edit**:
Turning a Task's row into the capture box (⋯ ▸ Quick edit, or E) to change its title and what it says. Words already in the old title are never read again; only new text is. Enter or clicking away saves (with Undo), Esc cancels.
_Avoid_: Inline edit, rename

**Repeat**:
A rule that brings a Task back after it is done: every N days, weeks (on chosen weekdays), months or years, counted either from the Due date or from the day it was done. Doing a repeating Task records that occurrence and moves the same Task to its next Due date. A repeating Task always has a Due date; if none was set, it takes the first day the rule matches; removing the date removes the Repeat. "After done" is only offered for an interval, never with chosen weekdays.
_Avoid_: Recurrence, rrule, habit

**Occurrence**:
One time a repeating Task was done, kept as a done copy of the Task linked to it and shown in Done. Only the latest occurrence can be reopened, which rewinds the Task to that date.
_Avoid_: Instance, run, completion

**Skip**:
Moving a repeating Task to its next Due date without recording an occurrence (History still shows it, as "Skipped"). The Checklist ticks are kept. It is what Won't do means on a repeating Task.
_Avoid_: Snooze, postpone

**Group by**:
How a view splits its open Tasks into groups: Today by Priority or List, Upcoming by day, Priority or List, the Inbox and a List by Priority or Due date, All by List, Priority or Due date, or no groups at all. Each view remembers its own choice. Overdue always stays on top, and Done is always by day.
_Avoid_: Filter, section

**Sort**:
How Tasks are ordered inside each group on a List page or the Inbox: Manual, Due date, Priority, Title or Recently added. Grouping happens first. Dragging is off while a sort other than Manual is on, and Manual brings back the drag order untouched. The Lists in the sidebar have their own sort: Manual, Name A–Z or Z–A, Most open or Oldest first.
_Avoid_: Order by

**Manual order**:
The order Tasks (and Lists) were dragged into, stored as one fractional-index key per Task and one per List. It is shared by every view, so reordering in Today also reorders the List.
_Avoid_: Position, rank, priority order

**Done earlier**:
The fold at the bottom of a List page holding Tasks done before today, "Done earlier · N". Tasks done today sit above it, struck through. A Setting changes this: all folded, all shown or hidden.

**Search**:
Finding Tasks by words in their title, notes or Comments, with the Journal's matching: word prefixes, case and accents ignored, quoted phrases and a "Similar matches" group for typos. Open Tasks come first, then Done and Won't do ones. The matching lives in `mnemora_core::search` and both Modules use it.
_Avoid_: Filter, find

**Focus ring**:
The mark (a ring and a tint) on the one Task row the keyboard is on. ↑ ↓ move it, and the Task keys act on the Task it sits on, never on a row the pointer merely rests on.
_Avoid_: Selection, cursor

**G chord**:
Pressing G and then T, U, I, A or D within a moment to go to Today, Upcoming, Inbox, All or Done.
_Avoid_: Leader key, vim keys

**Todo widget**:
The Android home-screen widget that shows one Todo view (Today, Upcoming, Inbox, All or a List), lets you complete its Tasks in place and add new ones. Each one placed on the home screen is set to its own view.
_Avoid_: Widget (alone), home card, launcher card

**Todo badge**:
The count beside Todo in the sidebar and on the phone tab bar: Tasks due today plus Overdue ones, red while anything is Overdue, hidden at zero. The Inbox and Today counts under Todo are separate.
_Avoid_: Notification count, unread

### Sync

**Sync**:
The optional Module, off by default, that keeps your own devices in step directly with each other, with no account or cloud service. It runs only while the app is open, and with it off the app behaves as if it did not exist.
_Avoid_: Backup, cloud, account

**Paired device**:
Another device of yours that this one trusts and keeps in step with. Pairing with any one device joins the whole group, so every device learns about the others. Forgetting a device cuts it off from all of them until it is paired again.
_Avoid_: Peer, client, node

**Pairing code**:
The short number one device shows and the other types to make them trust each other. It is single-use and short-lived.
_Avoid_: PIN, password, token

**Conflict**:
Text you wrote that two devices both rewrote in the same passage, so Sync cannot tell which to keep. It waits in the Sync Module for you to decide. Until then the newest version shows and nothing in Journal or Todo is marked. Only text (an Entry's body, a Task's title and notes, a Comment) can conflict.
_Avoid_: Clash, merge error, collision

**Resolved automatically**:
A difference between devices that Sync settled for you by keeping the newest value (a Priority, a Due date, a typed Address, a delete against an edit). It is only a log, with what was kept and what was replaced and a one-click "Use ‹other› instead", and never needs dismissing.
_Avoid_: Auto-merge, silent conflict

**Sync activity**:
The record of what Sync did: sessions with how much moved, pairing, joining, forgetting, pauses, failures, and Conflicts raised and resolved. It is kept for good, is shared by every device, and never holds your content.
_Avoid_: Sync log, history (that is a Task's)
