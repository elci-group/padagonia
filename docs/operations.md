# PADAGONIA Operations Guide

This document covers running, configuring, and observing PADAGONIA in production-like environments.

## Running with Docker

A multi-stage `Dockerfile` is provided at the repository root. It builds the
`padagonia` binary in a Rust builder stage and copies it into a slim
`debian:bookworm-slim` runtime image.

### Quick start with Docker Compose

```bash
docker compose up -d
```

Compose requires `PADAGONIA_API_KEY` and aborts if it is absent:

```bash
export PADAGONIA_API_KEY="$(openssl rand -hex 32)"
docker compose up -d
```

This builds the image, mounts `padagonia.docker.toml` as the runtime
configuration, and persists graph data in the `padagonia-data` Docker volume.

### Manual Docker build

```bash
docker build -t padagonia .
docker run -d \
  -p 7373:7373 \
  -v "$(pwd)/padagonia.docker.toml:/etc/padagonia/padagonia.toml:ro" \
  -v padagonia-data:/var/lib/padagonia/data \
  --name padagonia \
  padagonia
```

### Image defaults

- Exposed port: `7373`
- Binary location: `/usr/local/bin/padagonia`
- Default command: `server --config /etc/padagonia/padagonia.toml`
- Working directory: `/var/lib/padagonia`
- Runtime user: `padagonia` (unprivileged)

## Configuration

PADAGONIA loads configuration from a TOML file and from environment variables.

### Configuration file

Copy the example file and adjust it for your environment:

```bash
cp padagonia.docker.toml /etc/padagonia/padagonia.toml
```

Key settings:

| Section  | Key            | Description                                      |
|----------|----------------|--------------------------------------------------|
| `server` | `listen_addr`  | Socket address to bind, e.g. `0.0.0.0:7373`     |
| `server` | `api_key`      | Bearer token required for protected API routes   |
| `server` | `data_dir`     | Path to the store file                           |
| `storage`| `data_dir`     | Base directory for storage                       |
| `storage`| `default_graph`| Default graph file name                          |
| `logging`| `level`        | Log level (`trace`, `debug`, `info`, `warn`, `error`) |
| `hnsw`   | `m`            | HNSW maximum neighbor count                      |
| `hnsw`   | `ef_construction`| HNSW construction search depth                 |
| `hnsw`   | `ef`           | HNSW query search depth                          |
| `limits` | `request_body_bytes` | Maximum decoded HTTP request body size      |
| `limits` | `request_timeout_seconds` | End-to-end request timeout              |
| `limits` | `requests_per_second` / `request_burst` | Process-wide API token bucket |
| `limits` | `max_ingest_nodes` / `max_ingest_edges` | Synthetic ingest bounds       |
| `limits` | `max_bfs_depth` | Maximum traversal depth                         |
| `limits` | `max_vector_dimensions` | Maximum embedding/query dimensions        |
| `limits` | `max_vector_results` / `max_vector_ef` | Vector-search effort bounds    |

### Environment variable overrides

Any config value can be overridden with an environment variable using the
prefix `PADAGONIA__` and double underscores for nesting:

```bash
PADAGONIA__SERVER__LISTEN_ADDR=0.0.0.0:7373
PADAGONIA__SERVER__API_KEY=<at-least-16-random-bytes>
PADAGONIA__LOGGING__LEVEL=debug
```

## Health endpoints

The following public endpoints are exposed by the HTTP server:

| Endpoint | Method | Description                                              |
|----------|--------|----------------------------------------------------------|
| `/health`| GET    | Liveness probe; returns `{"status":"ok"}`                |
| `/ready` | GET    | Readiness probe; returns `{"status":"ready"}`            |
| `/metrics`| GET   | Prometheus metrics scrape endpoint                       |

Example probe:

```bash
curl http://localhost:7373/health
curl http://localhost:7373/ready
```

## Metrics

PADAGONIA exposes Prometheus-compatible metrics at `/metrics`. The metrics
recorder is installed at startup with a global `service="padagonia"` label.

The endpoint is public (no API key required) so Prometheus or compatible
scrapers can pull it directly. Example Prometheus job:

```yaml
scrape_configs:
  - job_name: 'padagonia'
    static_configs:
      - targets: ['localhost:7373']
    metrics_path: '/metrics'
```

CLI commands also emit counters such as `padagonia_cli_commands_total` when the
binary is run locally.

## API authentication

Protected API routes under `/api/v1` require a valid bearer token.

1. Set the token in the configuration:

   ```toml
   [server]
   api_key = "your-secret-token"
   ```

2. Include it in requests:

   ```bash
   curl -H "Authorization: Bearer your-secret-token" \
        http://localhost:7373/api/v1/stats
   ```

If the header is missing, malformed, or invalid, the server returns `401 Unauthorized`.

Authentication failures, accepted mutations, persistence outcomes, rejected
rate limits, and shutdown outcomes emit structured tracing events. Bearer
credentials and request payloads are never included. Every HTTP response carries
an `x-request-id`; a valid caller-supplied identifier is preserved for cross-service
correlation.

### Protected endpoints

| Endpoint        | Method | Description                              |
|-----------------|--------|------------------------------------------|
| `/api/v1/stats` | GET    | Returns node/edge/fact/label/relation counts |
| `/api/v1/ingest`| POST   | Generates a synthetic graph in memory      |

The exact route and schema contract is available at public
`GET /openapi.json` and checked in as [openapi.json](openapi.json).

## Durability, snapshots, and restore

Each successful mutation clones a consistent in-memory view and persists it off
the async executor before acknowledging the request. Saves write a unique
same-directory temporary file, flush and `fsync` it, atomically rename it over
the destination, and sync the parent directory on Unix. A torn write therefore
does not truncate the previous complete graph. This is single-file durability,
not a multi-operation transaction, WAL, replication protocol, or rollback
protection.

Create a validated snapshot without replacing an existing file:

```bash
padagonia snapshot --in /var/lib/padagonia/data/store.pad \
  --out /var/lib/padagonia/backups/store-$(date +%F).pad
```

Restore validates the snapshot before and after atomic replacement and requires
explicit overwrite consent:

```bash
systemctl stop padagonia
padagonia restore --in /var/lib/padagonia/backups/store-2026-07-31.pad \
  --out /var/lib/padagonia/data/store.pad --force
systemctl start padagonia
curl --fail http://127.0.0.1:7373/ready
```

Copy snapshots to separate failure domains, record retention, and test restore
regularly. Live-file deletion does not erase remote snapshots or logs.

## TLS reverse proxy

PADAGONIA intentionally does not terminate public TLS. Bind it to loopback and
put a maintained reverse proxy in front. A minimal nginx location is:

```nginx
location / {
    proxy_pass http://127.0.0.1:7373;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Request-Id $request_id;
    client_max_body_size 1m;
    proxy_connect_timeout 5s;
    proxy_read_timeout 35s;
    limit_req zone=padagonia burst=20 nodelay;
}
```

Configure certificates, HSTS, connection limits, network allowlists, and a
`limit_req_zone` according to the deployment environment. Keep the application
limit enabled as defense in depth; its token bucket is currently process-wide,
not per credential or source address.

## Security notes

- Replace the default `api_key` before exposing PADAGONIA to a network.
- Run the container as the provided unprivileged `padagonia` user.
- Bind the server to `0.0.0.0` only when running inside a container or behind a
  reverse proxy; use `127.0.0.1` for local development.
