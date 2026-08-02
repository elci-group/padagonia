# Threat Model

## Scope

This model covers the single-node PADAGONIA binary, its graph file, HTTP API,
configuration, metrics endpoint, container image, CI workflow, and release
artifacts. It assumes the host kernel and an authorized operator are trusted.
Replication, managed control planes, and multi-tenant isolation are outside the
current product boundary.

## Assets

- graph nodes, edges, embeddings, properties, and competing facts;
- provenance metadata and evidence references;
- availability and integrity of the graph file and its snapshots;
- bearer credentials and deployment configuration;
- release binaries, SBOMs, checksums, and provenance attestations;
- operational logs and metrics.

## Actors and trust boundaries

- Anonymous network callers may reach public health, readiness, metrics, and
  API parsing layers.
- Authenticated callers possess the configured bearer credential. Possession
  currently grants every API capability; it does not prove the `agent` field.
- Operators control configuration, filesystem permissions, process lifecycle,
  reverse proxies, backups, and credential rotation.
- CI runners and dependency publishers influence produced binaries.
- Graph content and persisted files remain untrusted even when created by a
  previously authenticated caller.

## Threats and controls

### Credential disclosure and bypass

Threats include blank/default credentials, timing comparisons, accidental log
capture, plaintext transport, copied configuration, and over-broad credentials.

Controls: startup fails on an empty key; comparison is constant-time for equal
length inputs; logs must never include authorization headers; the container runs
unprivileged; production guidance requires TLS termination and restrictive file
permissions. Scoped credentials and rotation without restart remain required
before shared-host deployment.

### Resource exhaustion

Threats include oversized JSON, huge synthetic ingest requests, extreme BFS
depth, vector dimensions or search effort, slow requests, concurrent writes,
metrics scraping, disk exhaustion, and decompression-style expansion during
decode.

Controls: storage frames have a hard allocation bound; HTTP requests have body,
time, and operation limits; mutation persistence runs off the async executor;
rate and concurrency limits reject excess work. Operators must still enforce
connection limits, disk quotas, and network policy at the reverse proxy or
orchestrator.

### Storage corruption and rollback

Threats include torn writes, truncation, checksum mismatch, semantic dangling
references, malicious frame lengths, replacement with an older valid snapshot,
and loss during backup or restore.

Controls: versioned headers, bounded frames, checksums, semantic validation,
trailing-byte rejection, same-directory atomic replacement, golden fixtures,
and explicit snapshot/restore verification. CRC32 detects accidental corruption
but is not a cryptographic authenticity mechanism. Rollback protection and
signed snapshots are not yet implemented.

### Data poisoning and provenance confusion

An authenticated caller can submit misleading labels, evidence, confidence,
timestamps derived by the server, embeddings, or self-referential graph data.
The caller-provided `agent` and `model` fields can impersonate display names.

Controls: documentation treats records as attributable claims rather than
truth; invalid numeric and structural inputs are rejected; competing facts are
retained. Future authenticated principal binding, policy constraints,
retractions, and evidence verification are needed for hostile collaboration.

### Sensitive-data leakage

Graph payloads can contain personal data, secrets, proprietary embeddings, or
identifiers. Leakage can occur through API responses, snapshots, logs, metrics,
core dumps, container layers, and release test fixtures.

Controls: protected graph routes require authentication; logs use identifiers
and outcomes rather than payloads or credentials; sample configs contain no
real secrets; snapshots inherit restrictive operator permissions. Deployers
must classify inputs and control backups because PADAGONIA cannot infer whether
a property is sensitive.

### Supply-chain compromise

Threats include malicious or abandoned crates, compromised action tags,
unexpected licenses or sources, build-runner compromise, and substituted
release binaries.

Controls: Cargo.lock is committed; RustSec and cargo-deny gate dependencies;
workflows use least-privilege permissions; releases publish checksums, SBOMs,
and GitHub/Sigstore build-provenance attestations. Consumers must verify the
attestation and repository identity; an attestation does not prove that the
source itself is safe.

### Operator and host compromise

An operator or process with write access can replace binaries, graph files,
configuration, or logs. Logical immutability does not defend against this.

Controls: unprivileged runtime, minimal image, filesystem permissions, external
log collection, verified releases, and separate backups reduce exposure. Host
hardening, secrets management, mandatory access control, and tamper-evident
remote audit storage are deployment responsibilities.

## Security invariants

1. No protected route accepts an empty, missing, malformed, or incorrect bearer
   credential.
2. No credential or full authorization header is written to logs or errors.
3. No declared frame or request body causes an allocation above its configured
   bound before rejection.
4. A successful mutation response follows successful durable snapshot replace.
5. A failed save preserves the last complete destination whenever the platform
   provides atomic same-filesystem rename.
6. Public endpoints expose process state, not graph contents.
7. Release artifacts are useful only when their checksum and provenance are
   verified against the expected repository and workflow identity.

## Residual risks and deployment requirements

- Put the server behind TLS and connection/rate enforcement.
- Store credentials in a secret manager or protected environment injection,
  not a committed TOML file.
- Restrict graph and snapshot directories to the service account.
- Export logs to access-controlled remote storage and alert on authentication
  failures, rejected limits, persistence failures, and checksum errors.
- Test restore regularly; an untested backup is not a recovery control.
- Do not use the current release as an adversarial multi-tenant service.

Review this model for every new route, storage version, credential role,
external connector, release mechanism, and replication mode.
