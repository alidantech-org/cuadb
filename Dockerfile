# Compile CSS and vendor HTMX using Bun; Bun is not included at runtime.
FROM oven/bun:1 AS assets
WORKDIR /app
COPY package.json ./
RUN bun install
COPY scripts/ ./scripts/
COPY assets/ ./assets/
COPY templates/ ./templates/
RUN bun run assets:build

# Build the Rust HTTP server and compile Askama templates.
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY Cargo.toml rust-toolchain.toml rustfmt.toml ./
COPY src/ ./src/
COPY templates/ ./templates/
RUN cargo build --release

# One small runtime container; no Node, Bun, nginx or database is bundled.
FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/cuadb ./cuadb
COPY --from=assets /app/public/ ./public/
ENV CUADB_PORT=4848\nENV CUADB_HOST=0.0.0.0
EXPOSE 4848
USER 10001:10001
CMD ["/app/cuadb"]
