# ============================================================================
# Stage 1: Planner — compute dependency recipe for cargo-chef caching
# ============================================================================
FROM rust:1.85-slim AS planner

WORKDIR /app

RUN cargo install cargo-chef --locked

COPY . .

RUN cargo chef prepare --recipe-path recipe.json

# ============================================================================
# Stage 2: Builder — cook deps (cached layer), then build the release binary
# ============================================================================
FROM rust:1.85-slim AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef --locked

# Restore dependency recipe from planner
COPY --from=planner /app/recipe.json recipe.json

# Cook dependencies — this layer is cached as long as recipe.json is unchanged
RUN cargo chef cook --release --recipe-path recipe.json

# Copy full source and build the workspace binary
COPY . .

RUN cargo build --release --package kaccy

# ============================================================================
# Stage 3: Runtime — minimal Debian image with only the compiled binary
# ============================================================================
FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user for the service
RUN groupadd --gid 1001 kaccy \
    && useradd --uid 1001 --gid kaccy --shell /bin/bash --create-home kaccy

COPY --from=builder /app/target/release/kaccy /usr/local/bin/kaccy

RUN chown kaccy:kaccy /usr/local/bin/kaccy

USER kaccy

# Runtime configuration — override via environment or Docker secrets
# DATABASE_URL   = postgres://user:pass@host:5432/dbname
# REDIS_URL      = redis://host:6379
# BITCOIN_RPC_URL= http://user:pass@host:8332
# JWT_SECRET     = <strong-random-secret>
ENV DATABASE_URL="" \
    REDIS_URL="" \
    BITCOIN_RPC_URL="" \
    JWT_SECRET=""

EXPOSE 8080

CMD ["/usr/local/bin/kaccy"]
