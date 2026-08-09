# Dreamsequence migration runbook

1. Write SQL changes to the compatibility outbox and project committed events
   into namespaced PADAGONIA transactions.
2. Rebuild a clean graph by replaying the journal, then compare canonical SQL
   read models with `compare_shadow_reads` for one complete retention period.
3. Record latency, storage, recovery, replay, authorization, and tenant
   isolation measurements using the benchmark-gate contract.
4. Switch analytics and engineering views to PADAGONIA only after the shadow
   diff is empty for the declared workload set.
5. Switch control-plane writes one route at a time, retaining SQL read-only as
   a rollback artifact.
6. Export and checksum SQL, verify PADAGONIA restore on a clean host, and run
   pairing, revocation, webhook idempotency, and rollback rehearsals.

No step permits deleting the SQL artifact before the release process signs the
restore and rollback evidence.
