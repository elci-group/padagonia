# Transaction journal

`TransactionJournal` is the durable mutation boundary for the first
PADAGONIA control-plane migration slice.

## Contract

- A transaction has a non-empty idempotency key and one or more mutations.
- A prepare record is written and synced before the mutation is applied.
- The mutation is applied to a working store clone, preserving atomicity for
  the batch.
- A commit record is written and synced before `commit` returns success.
- A repeated idempotency key returns the original `CommitResult` and does not
  apply the batch again.
- A truncated or incomplete prepare at the end of the journal is ignored on
  open; it was never acknowledged to a caller.
- Corrupt committed records fail recovery rather than being silently skipped.

The acknowledged-write durability boundary is the successful `sync_data` of a
commit record. A snapshot may be created afterward. To rebuild a clean store,
open the journal and call `replay` in sequence order, then persist a verified
snapshot. This is the current recovery point objective: zero loss for returned
successful commits, and possible loss of mutations whose commit had not yet
been acknowledged.

The working clone is intentionally retained for this prototype transaction
layer. Replacing it with page-level or copy-on-write mutation is required
before high-volume production cutover, as specified by the SQL-replacement
directive.
