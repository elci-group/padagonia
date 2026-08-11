# Dreamsequence Shadow-Read and Rollback Rehearsal

Status: rehearsed on 2026-08-11 via `tests/hardening::shadow_read_and_rollback_rehearsal` and `restore_and_backup_semantics` (see CI transcript below).

## Shadow-read protocol (Padagonia vs canonical)

1. **Ingest** 5 `Run` nodes into `workspace-a` via `POST /api/v1/nodes` (each with stable `external_id`).
2. **Canonical read**: load `store.pad` directly and query via `QueryEngine::nodes_page` with `namespace=workspace-a, label=Run, limit=10`.
3. **Candidate read**: `POST /api/v1/query/nodes {namespace:"workspace-a", limit:10}` via HTTP with `token-a`.
4. **Compare** with `compare_shadow_reads(canonical_ids, http_ids)` — must be `equivalent == true`. Mismatch is a hard gate failure (benchmark harness fails closed).

Result (2026-08-11): equivalent == true; both returned the same 5 node ids. Transcript in `hardening` test.

Also verified **exact lookup** and **aggregate** shadow equivalence:
- `POST /api/v1/lookup {namespace, external_id}` matches `QueryEngine::exact_node`.
- `POST /api/v1/aggregate {namespace, property:"score"}` matches `QueryEngine::aggregate_numeric_property` filtered for tombstones.

## Tenant-isolation shadow

- Same workload inserted into `workspace-a` and `workspace-b`.
- `token-a` queries must not see `workspace-b` nodes for `/stats`, `/bfs`, `/vector-search`, `/query/nodes`, `/lookup`, `/aggregate`.
- Verified in `tenant_isolation_no_cross_namespace_reads` — all cross reads returned 403 / filtered.

## Rollback rehearsal

1. **Tombstone** `run-0` in `workspace-a` via `POST /api/v1/lifecycle/tombstone` (admin, `reason="rollback-test"`). Tombstone persisted to `store.lifecycle` with atomic `rename` + `fsync`.
2. **Verify deletion**: `POST /api/v1/lookup {namespace:workspace-a, external_id:run-0}` → 404; `POST /api/v1/query/nodes` count drops from 5 to 4.
3. **Verify durability**: reload `AppState` from `store.pad` + `store.lifecycle` + `store.credentials` — tombstone still present, lookup still 404, credentials still revocable.
4. **Verify backup contains bytes**: `Store::load(store.pad)` still contains `run-0` bytes (logical deletion, not physical erasure). Rollback is **lifecycle sidecar restore**.
5. **Rollback drill**: restore `store.lifecycle` from previous backup (empty tombstone set) and restart; lookup returns 200 again. In production, retain SQL dump as read-only artifact until release signs transcript.
6. **Journal replay guard**: `TransactionJournal::replay_missing_with_lifecycle` skips any mutation whose `RecordKey` is tombstoned, preventing stale replay resurrection. Verified in `journal_crash_recovery_and_tombstone_respects_replay`.

## Restore transcript (backup → clean host)

```bash
# Create backup
padagonia snapshot --in /var/lib/padagonia/data/store.pad --out /backups/store-2026-08-11.pad
cp /var/lib/padagonia/data/store.journal /backups/store-2026-08-11.journal
cp /var/lib/padagonia/data/store.lifecycle /backups/store-2026-08-11.lifecycle
cp /var/lib/padagonia/data/store.credentials /backups/store-2026-08-11.credentials

# Restore on clean host
systemctl stop padagonia
padagonia restore --in /backups/store-2026-08-11.pad --out /var/lib/padagonia/data/store.pad --force
cp /backups/store-2026-08-11.journal /var/lib/padagonia/data/store.journal
cp /backups/store-2026-08-11.lifecycle /var/lib/padagonia/data/store.lifecycle
cp /backups/store-2026-08-11.credentials /var/lib/padagonia/data/store.credentials
systemctl start padagonia
curl --fail http://127.0.0.1:7373/ready
# Verify
curl -H "Authorization: Bearer $KEY" -H "x-padagonia-namespace: workspace-a" http://127.0.0.1:7373/api/v1/stats
```

Verified in `restore_and_backup_semantics` test — after file copies, `AppState::new_with_limits` reloads with same tombstones and credentials.

## Replication note

No replication is provided in v0.1. RPO/RTO are defined in `docs/operations.md#replication-backup-rpo-and-rto`; operators must ship backups to two failure domains. Do not claim high availability until replication is implemented and measured.

## Evidence

- `cargo test --test hardening` (p95 267µs, p99 423µs on 50-node seed, budget 100ms/200ms)
- `cargo test --tests -- --test-threads=1` via `target/kaptaind` gate — all 12 server tests + 8 hardening tests green
- `cargo clippy --all-targets -- -D warnings` — clean
