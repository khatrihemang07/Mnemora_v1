# Sync is peer to peer, merges per field and never purges

Sync keeps a person's own devices in step directly, with no server, account or cloud copy: devices find each other on the same Wi-Fi (mDNS) and over Tailscale (remembered addresses and an optional typed one), talk TCP with TLS 1.3 and pin each other's keys after pairing with an 8-digit code. Any device relays what it holds, so a group converges through any path. Changes are recorded in the change log (ADR 0008) with the origin device, a hybrid logical clock and a sequence, and merge field by field: different fields both survive, text is the replay of every stored record in clock order, merged three-way (by line, then by word) wherever a record was written against a different text than the replay holds, and only the same words changed on two devices raise a Conflict for the user. Every other clash takes the newest value and is logged as Resolved automatically. Nothing is ever purged, so a deleted row is its own tombstone and no old device can bring it back. Sync is a Module, off by default, and inert when off: no socket, mDNS, timer or task exists.

The decision is hard to reverse because the change log, the clock columns and "deleted rows stay forever" become the format every device must share, and it is easy to wonder why we did not use a hosted store.

## Considered Options

- **A server or cloud store**: rejected; the point is that the diary and tasks never leave the user's devices, and it means accounts and running something.
- **Whole-record last-writer-wins**: rejected; a Priority set on the phone and a Due date set on the Mac would cancel each other.
- **CRDT text for every field**: rejected; it needs a different storage format for all text and still cannot say "you rewrote the same sentence twice". Replaying the change log with each record's previous text (`prev`) is enough, and the rare real clash is shown to the user.
- **Conflicts for every field**: rejected; asking about a Priority is a chore. Only text people wrote is worth a question.
- **Keep purging deleted rows after 30 days**: rejected; a device away for longer would resurrect them, and tombstones would need their own expiry rules.
- **Relay or cloud rendezvous for devices on different networks**: rejected for v1; Tailscale covers it without us hosting anything.

## Amendment: what was built around the protocol

- **Group membership is synced data, signed.** Pairing with one member joins the whole group. Members that did not take part learn of the newcomer through a `sync_introduction` (signed by the introducing member; a signed `direct` flag marks the device it actually paired with, and only a direct introduction can undo a Forget). Forget is a signed `sync_revocation`; every member drops and refuses the key and a device that is paired again lifts it with a newer `lifted` decision. Members are equals: any paired member can introduce or lift. There is no remote wipe, and a forgotten device is not told, it shows Not nearby.
- **Remote text waits for the editor.** A record for a record or field being edited is stored, counted and relayed at once, but its values are held (`editing_sessions` leases of 120 s, renewed every 30 s by the editor) and merged when the editor releases. With Sync off nothing is held or written.
- **One logger per shared Activity event.** A session, pairing, pause or failure that both devices live through is logged by the device with the lower id; own events (rename, forget, leave) by the device itself; named events have deterministic ids so every device makes the same row.
- **Safety nets.** Every direct pairing backs up both databases first (own rotation of five). A peer more than 5 minutes ahead, or records stamped that far ahead, pause syncing rather than win every merge; so does a data-version mismatch. TCP keepalive notices a vanished peer in about 15 seconds.
- **Android runs the network only in the foreground** (plus a 5 s flush on leaving), only on a suitable link (mobile data needs the setting), and only with Sync on (`sync_power`). Closing the last desktop window ends the app, and with it Sync.
- **Schema.** Core migrations 3 to 6 (clock columns, `fields`, `editing_sessions`, `prev`), Todo 5 (Comment `edited`), and the Sync Module's own migrations hold the local and synced `sync_` tables.
