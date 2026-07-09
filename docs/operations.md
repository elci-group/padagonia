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

### Environment variable overrides

Any config value can be overridden with an environment variable using the
prefix `PADAGONIA__` and double underscores for nesting:

```bash
PADAGONIA__SERVER__LISTEN_ADDR=0.0.0.0:7373
PADAGONIA__SERVER__API_KEY=<secure-random-key>
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

### Protected endpoints

| Endpoint        | Method | Description                              |
|-----------------|--------|------------------------------------------|
| `/api/v1/stats` | GET    | Returns node/edge/fact/label/relation counts |
| `/api/v1/ingest`| POST   | Generates a synthetic graph in memory      |

## Security notes

- Replace the default `api_key` before exposing PADAGONIA to a network.
- Run the container as the provided unprivileged `padagonia` user.
- Bind the server to `0.0.0.0` only when running inside a container or behind a
  reverse proxy; use `127.0.0.1` for local development.
