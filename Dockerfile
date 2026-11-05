# syntax=docker/dockerfile:1

# 1. Bun compiles the CSS and JavaScript. It is not part of the final image.
FROM oven/bun:1 AS assets
WORKDIR /app
COPY . .
RUN bun install --frozen-lockfile && bun run assets:build

# 2. Rust builds one self-contained binary (the web assets are embedded in it).
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
COPY --from=assets /app/public ./public
RUN cargo build --locked --release && strip target/release/cuadb

# 3. Minimal runtime: the binary, a data volume, one HTTP port, no shell tools needed.
FROM debian:bookworm-slim AS runtime
LABEL org.opencontainers.image.title="CuaDB" \
      org.opencontainers.image.description="A local-first database explorer and SQL workspace for PostgreSQL and SQLite" \
      org.opencontainers.image.source="https://github.com/alidantech-org/cuadb" \
      org.opencontainers.image.licenses="MIT"
COPY --from=builder /app/target/release/cuadb /usr/local/bin/cuadb
ENV CUADB_HOST=0.0.0.0 \
    CUADB_PORT=4848 \
    CUADB_SQLITE_ROOT=/data/sqlite \
    CUADB_DATA_DIR=/data/app-state
RUN mkdir -p /data/sqlite /data/app-state && chown -R 10001:10001 /data
VOLUME ["/data"]
EXPOSE 4848
USER 10001:10001
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/cuadb", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/cuadb"]
