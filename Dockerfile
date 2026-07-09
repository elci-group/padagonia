# syntax=docker/dockerfile:1

# -----------------------------------------------------------------------------
# Builder stage
# -----------------------------------------------------------------------------
FROM rust:1.85-bookworm AS builder

WORKDIR /usr/src/padagonia

# Cache dependency resolution layer.
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY benches ./benches
COPY tests ./tests

RUN cargo build --release

# -----------------------------------------------------------------------------
# Runtime stage
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Install CA certificates so outbound TLS (e.g. Prometheus scrape over HTTPS)
# works if needed. libgcc1 is required by Rust binaries built with glibc.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# Create an unprivileged user for the service.
RUN groupadd -r padagonia \
    && useradd -r -g padagonia -d /var/lib/padagonia -s /sbin/nologin padagonia \
    && mkdir -p /etc/padagonia /var/lib/padagonia \
    && chown -R padagonia:padagonia /etc/padagonia /var/lib/padagonia

COPY --from=builder /usr/src/padagonia/target/release/padagonia /usr/local/bin/padagonia

USER padagonia
WORKDIR /var/lib/padagonia

EXPOSE 7373

ENTRYPOINT ["/usr/local/bin/padagonia"]
CMD ["server", "--config", "/etc/padagonia/padagonia.toml"]
