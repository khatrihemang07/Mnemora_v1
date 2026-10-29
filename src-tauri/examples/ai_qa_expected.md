# AI QA: expected facts for the built-in questions

These are the facts a correct answer must hold, for the ten built-in questions of `ai_qa` (`src-tauri/examples/ai_qa.rs`). They are derived by hand from `src-tauri/src/fixtures/recent_changes.json`, and the dated literals are cross-checked against `src-tauri/src/ai_fixture_tests.rs`, which fixes now at 2026-10-25 12:00 in America/New_York (a Sunday; yesterday is Saturday 24 October). Nothing here is computed by the harness: a reader judges each report against this file.

## How the fixture maps to a real date

The fixture stores each event as a day offset from **the day the seed ran** (`0` is that day, `-1` is yesterday). `make seed-sandbox` stamps them from that day, and the QA harness reads "now" at run time. So:

- **Yesterday** is offset `-1`, whatever the date.
- **Due dates** are offsets from the same day (`due: 1` means tomorrow). The overdue facts below hold for any date.
- **"This week" and "last week"** depend on the weekday of the day. With weeks starting on Monday (the app's default, `core.week_start` = `mon`), this week runs from the Monday on or before the day through the day itself, and last week is the seven days before that. The fixture was built so that the day is a Sunday; for another weekday, the windows below move.
- **Literal dates** in a question ("Wednesday 21 October") name a fixed day. The fixture's empty day (offset `-4`) is Wednesday 21 October only when the seed ran on Sunday 25 October.

Offsets to dates for a seed on **Sunday 25 October 2026** (the design date):

| Offset | Date | Offset | Date |
|---|---|---|---|
| -21 | Sun 4 Oct | -6 | Mon 19 Oct |
| -20 | Mon 5 Oct | -5 | Tue 20 Oct |
| -18 | Wed 7 Oct | -4 | Wed 21 Oct (no events) |
| -17 | Thu 8 Oct | -3 | Thu 22 Oct |
| -16 | Fri 9 Oct | -2 | Fri 23 Oct |
| -15 | Sat 10 Oct | -1 | Sat 24 Oct (yesterday) |
| -14 | Sun 11 Oct | 0 | Sun 25 Oct (today) |
| -13 | Mon 12 Oct | +1 | Mon 26 Oct |
| -12 | Tue 13 Oct | +2 | Tue 27 Oct |
| -11 | Wed 14 Oct | +3 | Wed 28 Oct |
| -10 | Thu 15 Oct | +4 | Thu 29 Oct |
| -9 | Fri 16 Oct | | |
| -8 | Sat 17 Oct | | |
| -7 | Sun 18 Oct | | |

Offset **-19** is the catch-up day (section 11): Tuesday 6 October for a seed on Sunday 25 October, and Saturday 10 October for a seed on Thursday 29 October (the live QA run of 2026-10-29). Offset -19 has no other events.

The fixture file's day numbers are what the Tools return as dates for the seed day. Entries are placed by when they were written (`at`); a Page date (`page`) does not move an Entry's change to the Page's day.

## Facts for a seed on Sunday 25 October 2026

### 1. "What did I do yesterday?" (Saturday 24 October)

Changes on 24 October (from `todo_changes` and `journal_changes` for that day):

- **Tasks created:** "Call the bank about the card" (09:02), "Reply to the Hartley email" (11:00), "Pick up the prescription" (13:30), "Reply to Anika about Sunday lunch" (18:00). Four in all.
- **Tasks done:** "Call the bank about the card" (10:45, after a comment at 10:44 that the card is blocked and a new one is due Tuesday), "Water the plants" (07:55, a daily repeat, occurrence of 24 Oct), "Pick up the dry cleaning" (23:50). Three in all.
- **Tasks moved or changed:** "Pay council tax" due date moved to Wednesday 28 Oct (09:30) with a comment "Moved to Wednesday, after payday." (09:35); "Email Priya the draft" moved from Inbox to Work (12:00) with a comment (12:05) that Priya asked for the numbers tab too; "Book the dentist" priority set to P1 (15:10) with a comment (15:12) about a slot on the 27th.
- **Comment:** "Renew passport" (13:05): "Sent the form off this morning. Still waiting on the photos."
- **Won't do:** "Learn the violin" (16:20). Not done.
- **Deleted:** "Buy birthday candles" (17:40).
- **Journal Entries written:** the night entry about a neighbour's dog barking (00:40), the Entry about office money and a draft at Will's (09:14), and the hill-top walk with the sheep (19:30).
- **Journal Entries edited:** the Entry about carrying L20 to Mr. Downing and Mr. Pierce, the surgeon (08:30, a sentence added about making up the balance).
- **Journal Entries deleted:** the Entry about the sermon at Mr. Messum's (11:02).

Counts for the day: Tasks created 4, done 3, won't do 1, moved 1, due date 2, priority 1, commented 6, deleted 1. Journal written 3, edited 1, deleted 1.

**Must mention** (any reasonable phrasing): Call the bank about the card (created and done), Water the plants (done), Pick up the dry cleaning (done), Reply to the Hartley email (created), Pick up the prescription (created), Reply to Anika about Sunday lunch (created), the council tax move to Wednesday, the passport comment, the dog-barking Entry, the hill-top walk Entry.

**Must not mention as yesterday's:** the Friday walk Entry (23 Oct, 07:30), the Sunday walk Entry (25 Oct), "Send the invoice to Harlow & Co" (not changed on 24 Oct), "Ring Grandma back" (25 Oct), "Learn the violin" as done (it was won't do).

### 2. "What did I get done last week?" (Monday 12 to Sunday 18 October)

- **Done:** "Email the landlord about the boiler" (Mon 12 Oct, 12:30) and "Order new running shoes" (Sun 18 Oct, 19:05). Two tasks.
- **Won't do (not done):** "Check the car insurance renewal" (Wed 14 Oct). It must not be listed as done.
- **Not done:** "Fix the kitchen tap washer" (a comment on 18 Oct says the washer is bought, but the Task is still open), "Sort the spare room", "Renew passport" (open).

Last week's journal Entries (written, not tasks): the walk to Holford (13 Oct), the Mr. Crew's bedside Entry about the secluded members (14 Oct), the Mr. Crew's Entry about the coach and the pictures (15 Oct), the office and Will's Entry (17 Oct), the bright-sunshine Entry about the sea and the moon (18 Oct).

**Must not mention as done:** "Check the car insurance renewal", "Fix the kitchen tap washer", "Take the bins out" (its Done is on 20 Oct, this week), "Water the plants" (done 22 and 24 Oct, this week), "Return the library books" (done 22 Oct, this week).

### 3. "What's overdue?"

Open Tasks whose due date has passed as of today (Sunday 25 Oct):

- **"Fix the kitchen tap washer"**, due Sun 18 Oct, still open.
- **"Send the invoice to Harlow & Co"**, due Fri 23 Oct, still open (a comment on 22 Oct says they will pay on Monday; the Task was not completed).

**Must not list** (not overdue): "Pay council tax" (due Wed 28), "Renew passport" (due Thu 29), "Draft the Q4 report" (due Thu 28), "Call the vet about the booster" (due today), "Ring Grandma back" (due today), "Reply to the Hartley email" and "Pick up the prescription" (due Mon 26), "Book the dentist" and "Send Mum a birthday card" (due Tue 27), "Take the bins out" (repeating, its 20 Oct occurrence is done), "Water the plants" (repeating, done on 24 Oct), "Pick up the dry cleaning" (done), "Return the library books" (done), "Order new running shoes" (done), "Email the landlord about the boiler" (done). "Check the car insurance renewal" and "Learn the violin" are won't do, not overdue.

### 4. "What did I complete yesterday versus what did I only create?"

- **Completed yesterday (24 Oct):** "Water the plants", "Call the bank about the card", "Pick up the dry cleaning". Three.
- **Created yesterday:** "Call the bank about the card" (also completed yesterday), "Reply to the Hartley email", "Pick up the prescription", "Reply to Anika about Sunday lunch". Four.
- **Only created (not completed yesterday):** "Reply to the Hartley email", "Pick up the prescription", "Reply to Anika about Sunday lunch". The last one was completed at 00:10 on Sunday 25 October, which is not yesterday, so an answer may say it was finished the next morning.

Journal Entries are not "completed" or "created" for this question, so an answer that counts Entries as completions is wrong.

### 5. "What was on my mind in my journal this week?" (Monday 19 to Sunday 25 October)

Entries written or changed this week:

- Mon 19 Oct, 08:05: sent for by Mr. Downing, who had made the writer a Clerk of the Council; unsure whether to thank him.
- Tue 20 Oct, 22:10: finishing accounts, writing to his Lord, taking leave of Mr. Sheply and the keys of the house.
- Fri 23 Oct, 07:30: a walk in the cold clear evening, the sea of a sober grey, the sheep-bell in the coombe.
- Fri 23 Oct, 20:05 (edited Sat 24 Oct, 08:30): carrying L20 to Mr. Downing, taking Mr. Pierce the surgeon to the Axe; the edit adds "Thence to my office and there did nothing but make up my balance."
- Sat 24 Oct, 00:40: awake for hours with a neighbour's dog barking, slept late, took physic, stayed in all day.
- Sat 24 Oct, 09:14: an office morning, drinking a draft at Will's, counting the excise money, then taking the wife to Mr. Pierce's, slowed by her new pattens.
- Sat 24 Oct, 19:30: a walk on the hill-tops, the sheep tracks, the woodman and his pony, the sea under clouds.
- Sun 25 Oct, 09:20: a walk from seven o'clock, the moon through the cloud, the oak shadows.

Themes an answer may draw from these: work and money at the office and the Council, family and the house, walks and the weather, and a broken night's sleep. The sermon Entry (22 Oct) was deleted on 24 Oct and must not be presented as a current Entry.

### 6. "What happened on Wednesday 21 October?" (the empty day)

**Nothing.** The fixture has no Entries, Task changes, due dates or comments on 21 October. A correct answer says so plainly and invents nothing. An answer that lists Tasks from other days, or Entries from 20 or 22 October, is wrong.

### 7. "Hi"

A greeting in English, with no Tool calls and no claim about the person's data. An answer that lists Tasks or Entries here is wrong.

### 8. "What exactly did I write in the Entry about the dog barking?"

The Entry is dated Saturday 24 October, written at 00:40. Its text must be quoted exactly:

> Having been exceedingly disturbed in the night with the barking of a dog of one of our neighbours that I could not sleep for an hour or two, I slept late, and then in the morning took physic, and so staid within all day.

The answer should quote it word for word, and may cite the Entry's id. Paraphrase is wrong here: the question asks for the exact words.

### 9. "Any updates on the passport?"

Task "Renew passport", due Thu 29 Oct, still open. Its comments, oldest first:

- Thu 15 Oct, 18:40: "Photos first, then the form."
- Sat 24 Oct, 13:05: "Sent the form off this morning. Still waiting on the photos."

Correct answer: the form was sent, and the photos were still awaited as of the latest comment. The Task is not done. An answer that says the passport is renewed, or the photos arrived, is wrong.

### 10. "How many tasks did I finish this week?" (Monday 19 to Sunday 25 October)

- **Tasks finished: 6.** "Take the bins out" (done Tue 20 Oct, 19:40, a repeat), "Return the library books" (Thu 22 Oct), "Water the plants" (done twice: Thu 22 Oct and Sat 24 Oct, both occurrences of a daily repeat), "Call the bank about the card" (Sat 24 Oct), "Pick up the dry cleaning" (Sat 24 Oct), "Reply to Anika about Sunday lunch" (Sun 25 Oct, 00:10).
- **Completions: 7**, counting the two occurrences of "Water the plants". An answer of 6 tasks, or of 7 completions with that explained, is correct. The change Tool counts the same thing: `done: 7` for 19 to 25 October.
- **Not finished:** "Order new running shoes" (finished Sun 18 Oct, last week), "Check the car insurance renewal" and "Learn the violin" (won't do), "Buy birthday candles" (deleted), the 22 Oct skip of "Water the plants".

### 11. The catch-up day (offset -19: Tuesday 6 October for a seed on 25 October; Saturday 10 October for a seed on 29 October)

This is the one day with more changes than a default batch holds (100 rows). It was written so that paging matters: a question about this day can be answered completely only by following `next_offset`. The counts below are the literals of `src-tauri/src/ai_catchup_tests.rs` and `ai_fixture_tests.rs`, derived by hand from the fixture events (a generator script counted the same numbers before the events were written).

**Task changes** (`todo_changes` for the day, default limit 100): **134 rows in all**, shown 100 in the first batch and 34 in the second (`next_offset` 100, then null). Counts for the whole day: **created 61, done 53, won't do 1, moved 1, deleted 1, commented 16, repeat set 1**; due 0, priority 0, edited 0, skipped 0.

- **Created and ticked:** 52 of the 60 everyday Tasks were created and then ticked on the same day, the ticks between 07:10 and 15:30. The first of them is "Clean the oven" (created 07:00, commented 07:05, done 07:10). "Scan the old photos" is row 100, done at 15:30, the last row of the first batch.
- **Repeating Task:** "Change the bedding" is created at 18:20, set to repeat every week (a repeat row), and done at 18:30 as "Done (occurrence of 6 Oct)", or "of 10 Oct" for the live seed (the day's own date).
- **Won't do:** "Defrost the freezer" (17:50).
- **Moved:** "Mop the bathroom floor" from Home to Work (18:00). It is created in Home at 17:55.
- **Deleted:** "Air the duvet" (18:10). It must not appear in any list or search afterwards.
- **Still open at the end of the day:** "Check the tyre pressure", "Restock the first-aid kit", "Email the plumber back", "Clear the browser downloads", "Mop the bathroom floor" and "Set up the printer" (six Tasks).
- **Comments:** 16, on the first 16 Tasks, each a few words (for example "Done after the kids were in bed." on "Clean the oven" at 07:05).

**Journal rows** (`journal_changes` for the day): **5 rows, complete in one call** (no `next_offset`, no note). Counts: **written 4, edited 1, deleted 0**.

- 09:25, written, 25 words: a bad night, tea by the window until it got light, the empty street and the bin lorry.
- 12:30, written, 32 words: lunch in the park with Sam about the new job, "I think I already know what I want, I just have not said it yet."
- 15:05, written, 20 words: a long queue at the post office, a walk home through the market, bread and lemons.
- 16:00, **edited** (the lunch Entry): the last sentence becomes "...and I said so out loud for the first time." Its row carries the first 120 characters of the text.
- 17:35, written, 20 words: a quiet evening, the book finished on the sofa, bed before eleven.

**What a complete answer must mention:** the day's scale, stated in counts (61 Tasks created and 53 done, or the total of 134 changes), or the answer names that it read the whole day (every batch followed). It must mention the Entries (the lunch with Sam about the new job, at least) and that the repeating Task "Change the bedding" was done. It must not state a count from the first batch alone as the day's total (for example "100 changes").

**What an incomplete answer looks like:** a Run that reads only the first batch and answers must say so in its own words, such as "I read the first 100 of 134 changes for that day; the rest is not included." The app adds its own note under the answer, "Based on the first 100 of 134 changes (Tasks); the rest was not read.", and that note is correct and expected. An answer that stops after batch 1 and gives no sentence of incompleteness, and no app note, is a failure (see the checks below).

## Facts for a seed on another date

The relative facts above (yesterday, due dates, overdue, the Entries and Tasks, the passport, the dog Entry) hold for any seed date, since they are offsets. Two things move:

1. **The week windows.** For a seed on Wednesday 28 October 2026 (today, when this file was written), this week is Monday 26 October to Wednesday 28 October, which is offsets -2 to 0; last week is Monday 19 to Sunday 25 October, offsets -9 to -3. The week-based answers change accordingly:
   - **"What did I get done last week?"** (19 to 25 Oct, offsets -9 to -3): "Order new running shoes" (Wed 21 Oct), "Take the bins out" (Fri 23 Oct), "Return the library books" (Sun 25 Oct), "Water the plants" (Sun 25 Oct).
   - **"How many tasks did I finish this week?"** (26 to 28 Oct, offsets -2 to 0): "Water the plants" (Tue 27 Oct), "Call the bank about the card" (Tue 27 Oct), "Pick up the dry cleaning" (Tue 27 Oct), "Reply to Anika about Sunday lunch" (Wed 28 Oct, 00:10). Four tasks.
   - **"What was on my mind in my journal this week?"** (26 to 28 Oct): the Monday 26 Oct walk and the Monday 26 Oct evening Entry about carrying L20 (with its Tuesday edit), the Tuesday 27 Oct Entries (the dog barking, the office morning, the hill-top walk), and the Wednesday 28 Oct walk.
2. **The literal date in question 6.** "Wednesday 21 October" is offset -7 for that seed, which has events (the running shoes done, a comment on the tap, an Entry at 19:45 about sunshine, a comment on the spare room). The empty day is then Saturday 24 October. The question as written is not the empty day unless the seed ran on Sunday 25 October, so judge question 6 against the fixed date: the correct answer lists what the fixture holds on 21 October, or says the fixture holds nothing for that date, which for this seed is the literal fact.

Questions 1, 3, 4, 7, 8 and 9 need no change. Question 10's expected count changes with the window. Question 2's expected Tasks change with the window.

## Checks an answer must pass, whatever the date

- It names only Tasks and Entries that a Tool returned in the Run, and cites their ids in brackets.
- It states "nothing" where the fixture holds nothing.
- It does not present a deleted Entry as current, or a won't do Task as done.
- Question 8 quotes the Entry exactly.
- Question 7 makes no Tool call.
- For the catch-up day (section 11), a Run either follows `next_offset` to the end (the last call has no `next_offset`) or says the answer is incomplete. Stopping after the first batch with neither is a failure.
