# Bun builds local CSS/JS assets; it is absent from the final container.
FROM oven/bun:1 AS assets
WORKDIR /app
COPY . .
RUN bun install --frozen-lockfile
RUN bun run assets:build

# Build Rust and compile Askama templates. Existing Cargo.lock is respected.
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --locked --release

# One runtime container, one HTTP port: no Node, Bun, Nginx or database.
FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/cuadb ./cuadb
COPY --from=assets /app/public/ ./public/
ENV CUADB_PORT=4848
ENV CUADB_HOST=0.0.0.0
ENV CUADB_SQLITE_ROOT=/data
VOLUME ["/data"]
EXPOSE 4848
USER 10001:10001
CMD ["/app/cuadb"]
