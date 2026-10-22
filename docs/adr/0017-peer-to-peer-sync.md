# Sync is peer to peer, merges per field and never purges

Sync keeps a person's own devices in step directly, with no server, account or cloud copy: devices find each other on the same Wi-Fi (mDNS) and over Tailscale (remembered addresses and an optional typed one), talk TCP with TLS 1.3 and pin each other's keys after pairing with an 8-digit code. Any device relays what it holds, so a group converges through any path. Changes are recorded in the change log (ADR 0008) with the origin device, a hybrid logical clock and a sequence, and merge field by field: different fields both survive, text is merged three-way by line and then by word, and only the same words changed on two devices raise a Conflict for the user. Every other clash takes the newest value and is logged as Resolved automatically. Nothing is ever purged, so a deleted row is its own tombstone and no old device can bring it back. Sync is a Module, off by default, and inert when off: no socket, mDNS, timer or task exists.

The decision is hard to reverse because the change log, the clock columns and "deleted rows stay forever" become the format every device must share, and it is easy to wonder why we did not use a hosted store.

## Considered Options

- **A server or cloud store**: rejected; the point is that the diary and tasks never leave the user's devices, and it means accounts and running something.
- **Whole-record last-writer-wins**: rejected; a Priority set on the phone and a Due date set on the Mac would cancel each other.
- **CRDT text for every field**: rejected; it needs a different storage format for all text and still cannot say "you rewrote the same sentence twice". Three-way merge over snapshots we already keep is enough, and the rare real clash is shown to the user.
- **Conflicts for every field**: rejected; asking about a Priority is a chore. Only text people wrote is worth a question.
- **Keep purging deleted rows after 30 days**: rejected; a device away for longer would resurrect them, and tombstones would need their own expiry rules.
- **Relay or cloud rendezvous for devices on different networks**: rejected for v1; Tailscale covers it without us hosting anything.
