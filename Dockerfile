# ── Build stage ──────────────────────────────────────────────────────────────
FROM debian:bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    ca-certificates \
    cmake \
    ninja-build \
    python3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock* ./
COPY crates/open-core/Cargo.toml crates/open-core/Cargo.toml
COPY crates/open-cli/Cargo.toml crates/open-cli/Cargo.toml
COPY crates/open-debug/Cargo.toml crates/open-debug/Cargo.toml
COPY crates/open-cdp/Cargo.toml crates/open-cdp/Cargo.toml
COPY crates/open-kg/Cargo.toml crates/open-kg/Cargo.toml

RUN mkdir -p crates/open-core/src && echo "" > crates/open-core/src/lib.rs && \
    mkdir -p crates/open-cli/src && echo "fn main() {}" > crates/open-cli/src/main.rs && \
    mkdir -p crates/open-debug/src && echo "" > crates/open-debug/src/lib.rs && \
    mkdir -p crates/open-cdp/src && echo "" > crates/open-cdp/src/lib.rs && \
    mkdir -p crates/open-kg/src && echo "" > crates/open-kg/src/lib.rs

RUN cargo +nightly build --release 2>/dev/null || true

COPY . .
RUN touch crates/open-core/src/lib.rs crates/open-cli/src/main.rs \
      crates/open-debug/src/lib.rs crates/open-cdp/src/lib.rs crates/open-kg/src/lib.rs
RUN cargo +nightly build --release --bin open-browser

# ── Runtime stage ────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libstdc++6 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 1000 open \
    && useradd --uid 1000 --gid open --create-home open

WORKDIR /home/open

COPY --from=builder /app/target/release/open-browser /usr/local/bin/open-browser

# CDP server port
EXPOSE 9222

# Health check via CDP HTTP discovery endpoint
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -sf http://127.0.0.1:${PORT:-9222}/json/version || exit 1

USER open

# Default: start CDP server bound to all interfaces. Override with `docker run ... <subcommand> [args]`
ENTRYPOINT ["open-browser"]
CMD ["serve", "--host", "0.0.0.0"]
