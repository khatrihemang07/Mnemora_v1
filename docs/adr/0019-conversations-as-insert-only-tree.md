# Conversations are insert-only entries in a tree, synced by union

A Conversation is stored as a tree of entries that are written once and never edited: a question, an answer, a Tool result or a Model change, each pointing at the entry it followed. Retry and Edit add sibling entries, and Branch from earlier chooses an earlier point, so two devices that continue the same point simply produce two Branches. The entries are synced by union, following the insert-only precedent of Sync activity (ADR 0017), and the Conversation's title is an ordinary synced text field. The trade-off is that nothing is ever edited or purged, so a Conversation only grows and deletion is a soft mark with Undo, in exchange for merges that cannot lose a message and need no conflict prompts. Tool results copy Journal and Todo text into these tables on the person's own devices, which we accept because the data never leaves them.

## Considered Options

- **One mutable transcript per Conversation, merged as text**: rejected; two devices continuing the same point would raise a Conflict on a transcript that is not really text, and it cannot express Branches.
- **Whole-Conversation last-writer-wins**: rejected; a continuation on the phone would silently drop the one made on the Mac.
- **Keep Conversations on the device that made them**: rejected; a Conversation must be continuable on another device with that device's own Provider.
- **Store only the answer text, not the Tool results**: rejected; a later question about the same Run could not check its Sources, and the history could not be replayed.
