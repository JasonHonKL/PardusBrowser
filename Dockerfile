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

COPY Cargo.toml Cargo.lock* ./
COPY crates/pardus-core/Cargo.toml crates/pardus-core/Cargo.toml
COPY crates/pardus-cli/Cargo.toml crates/pardus-cli/Cargo.toml
COPY crates/pardus-debug/Cargo.toml crates/pardus-debug/Cargo.toml
COPY crates/pardus-cdp/Cargo.toml crates/pardus-cdp/Cargo.toml

RUN mkdir -p crates/pardus-core/src && echo "" > crates/pardus-core/src/lib.rs && \
    mkdir -p crates/pardus-cli/src && echo "fn main() {}" > crates/pardus-cli/src/main.rs && \
    mkdir -p crates/pardus-debug/src && echo "" > crates/pardus-debug/src/lib.rs && \
    mkdir -p crates/pardus-cdp/src && echo "" > crates/pardus-cdp/src/lib.rs

RUN cargo +nightly build --release 2>/dev/null || true

COPY . .
RUN touch crates/pardus-core/src/lib.rs crates/pardus-cli/src/main.rs
RUN cargo +nightly build --release --bin pardus-browser

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libstdc++6 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/pardus-browser /usr/local/bin/pardus-browser

ENTRYPOINT ["pardus-browser"]
