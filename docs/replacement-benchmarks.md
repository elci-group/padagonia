# SQL-replacement benchmark contract

The replacement claim is evaluated only from measurements captured on the same
hardware, dataset, and versioned workload for PADAGONIA, SQLite, and a
representative PostgreSQL deployment.

Each workload records p50/p95/p99 latency, sustained writes, storage density,
recovery time, acknowledged-write loss, tenant-isolation violations, and
unbounded-query observations. `benchmark_gate::evaluate` turns those metrics
into a pass/fail report. Safety thresholds default to zero tolerated
acknowledged-write loss, cross-tenant access, and unbounded queries; latency
thresholds must be declared by the release process.

The current repository contains the gate and workload contract, not a claim
that PADAGONIA has already won the comparison. A release must attach raw
results, hardware details, dataset checksums, and the replay/restore logs.
