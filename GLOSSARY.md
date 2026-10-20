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
A user setting stored with the data, so it follows the data when synced, such as theme or time format.
_Avoid_: Option, config

**Device setting**:
A setting that belongs to one device and is never synced, such as window size or the data folder.
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

