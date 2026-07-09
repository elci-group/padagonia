# PADAGONIA Roadmap to S-Grade Maturity

This roadmap turns PADAGONIA from a working prototype into a commercially mature, S-grade product. Each phase builds on the last and is gated by the `deliver.toml` acceptance suite plus phase-specific checks.

## Maturity grading scale

- **S** — Enterprise-ready: secure, observable, scalable, legally clear, commercially deployable.
- **A** — Production-ready for single-tenant or self-hosted deployments.
- **B** — Solid open-source project with good DX and CI.
- **C** — Working prototype.

Current state: **C+**.

---

## Phase 1 — Foundation (→ B)

Goal: make the project look and behave like a serious open-source product.

- [x] `LICENSE-MIT` / `LICENSE-APACHE` (dual license)
- [x] `CONTRIBUTING.md`
- [x] `CODE_OF_CONDUCT.md`
- [x] `SECURITY.md`
- [x] `CHANGELOG.md` (Keep a Changelog format)
- [x] `Cargo.toml` metadata: authors, description, repository, license, keywords, categories, rust-version
- [x] Configuration layer (`padagonia.toml`/env) using the `config` crate
- [x] Structured logging with `tracing`
- [x] Prometheus metrics endpoint (`/metrics`)
- [x] HTTP server skeleton (`padagonia server`) with Axum
- [x] Health/readiness endpoints (`/health`, `/ready`)
- [x] API-key authentication middleware
- [x] Dockerfile, `.dockerignore`, `docker-compose.yml`, and `padagonia.docker.toml`
- [x] CI workflow extended with Docker build and `cargo audit`
- [x] Release workflow building binaries for Linux, macOS, and Windows

## Phase 2 — Persistence & Operations (→ B+)

Goal: move from "demo in memory" to "ops-grade data store".

- [ ] Write-ahead log (WAL) for durability
- [ ] Snapshot/restore commands (`padagonia snapshot`, `padagonia restore`)
- [ ] Pluggable storage backends (in-memory, Sled, RocksDB)
- [ ] Backup/restore HTTP API
- [ ] Graceful shutdown and in-flight request draining
- [ ] Structured error responses and OpenAPI spec
- [ ] Integration tests using the HTTP API and Testcontainers
- [ ] `cargo-deny` license/advisory policy

## Phase 3 — Security & Compliance (→ A)

Goal: pass a security review and support enterprise procurement.

- [ ] TLS for server (native-tls or rustls)
- [ ] Encryption at rest option for backend stores
- [ ] RBAC: roles (admin, editor, viewer) and API key scoping
- [ ] Audit log of all mutations and auth events
- [ ] Input validation, size limits, rate limiting
- [ ] Dependency vulnerability scanning in CI (`cargo audit`)
- [ ] Security review guide and threat model in `SECURITY.md`
- [ ] Signed release artifacts and SBOM

## Phase 4 — Scalability (→ A+)

Goal: handle production workloads beyond a single node.

- [ ] Columnar / GPU-friendly storage layout prototypes
- [ ] Replication: leader-follower sync protocol
- [ ] Sharding by semantic cluster
- [ ] Distributed agent scheduler (Raft/consensus for task ownership)
- [ ] Benchmark suite expanded to multi-node scenarios
- [ ] Performance regression gates in CI

## Phase 5 — Commercial SaaS Layer (→ S)

Goal: turn Pro from a marketing page into a real managed product.

- [ ] Multi-tenant namespace isolation in the server
- [ ] Organization/member management API
- [ ] Stripe billing integration and usage metering
- [ ] Entitlement/feature flags per plan
- [ ] Admin dashboard (web UI)
- [ ] SLI/SLO metrics and public status page
- [ ] Support ticketing integration
- [ ] SOC 2 / ISO 27001 readiness documentation
- [ ] Hosted deployment automation (Terraform / Helm)

## Phase 6 — Ecosystem (→ S+)

Goal: become the default substrate for agentic systems.

- [ ] Language clients (Python, TypeScript, Go)
- [ ] Protocol adapters (MCP, A2A, OpenAI Agents SDK)
- [ ] Managed cloud marketplace listing
- [ ] Community plugin registry
- [ ] Certification/training program

---

## Execution rules

1. Each phase must keep `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and the full `deliver.toml` suite green.
2. New dependencies must be justified and pinned in `Cargo.lock`.
3. All user-facing changes need docs/website updates.
4. Commit via `kaptaind` when possible; otherwise use conventional commits.
