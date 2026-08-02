# Design Principles

PADAGONIA is an epistemic substrate, not an oracle. It stores claims made by
actors, the context attached to those claims, and graph structure that makes
them useful. It does not turn recorded data into truth merely by persisting it.

## 1. Claims remain attributable

Every node and edge carries provenance. `agent` and `model` identify the
asserting process as supplied by the caller; they are attribution fields, not
authenticated identities. `evidence` contains references or descriptions, not
proof that the referenced material exists or supports the claim.

Future identity work must bind authenticated principals to assertions without
silently treating caller-provided display names as authority.

## 2. Confidence is not probability by default

Confidence is a caller-supplied score. PADAGONIA preserves and compares it but
does not assume calibration across agents, models, time, or domains. Consumers
must not aggregate confidence as probability unless a documented calibration
model makes that operation valid.

Non-finite confidence values are invalid at system boundaries. Competing facts
remain visible so uncertainty is represented rather than overwritten.

## 3. Append-only values support correction, not historical erasure

Stored node and edge values are exposed read-only and new assertions append to
the graph. Correction should be modeled as a new attributed assertion or a
future explicit retraction record, preserving why the system changed its mind.

This is logical immutability, not tamper-evident storage. An operator with file
access can replace a graph. Cryptographic history would require signed records
or a hash-linked journal and is not currently claimed.

## 4. Permanence has ethical limits

Append-only history conflicts with privacy, retention, and deletion duties.
Deployers must minimize personal or secret data, set retention outside the
current engine, and understand that deleting a live file does not erase copies
in snapshots, logs, or remote backups. Namespace-scoped redaction and auditable
compaction are prerequisites for privacy-sensitive multi-tenant use.

## 5. Ontology awareness is distinct from reasoning

Interned labels, relations, and keys provide a stable vocabulary and compact
indexes. PADAGONIA does not yet implement schemas, constraints, subsumption,
entailment, or automated ontology alignment. Documentation uses
"ontology-aware" for the implemented capability and reserves stronger claims
for measured reasoning features.

## 6. Bound every trust boundary

Files, configuration, HTTP bodies, graph sizes, search effort, dimensions, and
timestamps are untrusted inputs. Each boundary needs a size or effort limit, a
stable failure, and enough structured context to reconstruct an incident
without leaking secrets.

## 7. Acknowledged writes need a recovery story

The single-node server persists after each accepted mutation. A response is not
successful until persistence completes. Same-directory atomic replacement
protects the last complete snapshot from torn overwrite; it does not provide
multi-node consensus or uninterrupted availability.

## 8. Determinism is a product property

Fixtures, synthetic workloads, HNSW construction, projections, and migrations
use explicit seeds and stable ordering where callers can observe results.
Nondeterminism must be isolated, documented, and measured rather than hidden by
wide benchmark tolerances.

## 9. Observability must preserve dignity and secrecy

Logs describe operations, outcomes, correlation identifiers, and safe object
identifiers. They never record bearer credentials or full sensitive payloads.
Metrics should reveal system behavior without becoming a side channel for graph
contents or tenant activity.

## 10. Prefer narrow, composable authority

Authentication proves possession of a credential; authorization determines
what that credential may do. The present single-key boundary is suitable only
for controlled single-tenant deployment. Reader, writer, and administrator
roles plus namespace quotas are required before shared-host claims.

## 11. Recovery outranks novelty

A smaller engine with explicit limits, verified storage, and practiced restore
procedures is more advanced than a feature-rich system with ambiguous failure
semantics. Correctness, recovery, and security therefore outrank client count,
protocol count, and synthetic throughput.

## Non-goals of the current single-node release

- distributed consensus, replication, or automatic failover;
- cryptographically verified provenance or caller identity;
- general ontology reasoning or truth adjudication;
- privacy-complete erasure across external backups;
- adversarial multi-tenant isolation;
- transactional mutation across multiple HTTP requests.
