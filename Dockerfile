# --- Build stage ---
FROM rust:1-slim-trixie AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

# --- Runtime stage ---
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/github-trending-summary /usr/local/bin/github-trending-summary
# AppConfig reads config/default.toml relative to the current directory.
COPY config ./config
ENV RUN_MODE=production
ENTRYPOINT ["github-trending-summary"]
