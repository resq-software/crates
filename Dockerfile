# Copyright 2026 ResQ
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# ── Stage 1: cargo-chef planner ─────────────────────────────────────────────
FROM rust:1-slim AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

# ── Stage 2: dependency recipe ───────────────────────────────────────────────
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 3: build ───────────────────────────────────────────────────────────
FROM chef AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies before copying source
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN cargo build --release --workspace

# ── Stage 4: minimal runtime ─────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 1000 -s /bin/sh resq
WORKDIR /app

# Copy every workspace binary
COPY --from=builder /app/target/release/resq         /usr/local/bin/resq
COPY --from=builder /app/target/release/resq-bin     /usr/local/bin/resq-bin
COPY --from=builder /app/target/release/resq-clean   /usr/local/bin/resq-clean
COPY --from=builder /app/target/release/resq-flame   /usr/local/bin/resq-flame
COPY --from=builder /app/target/release/resq-perf    /usr/local/bin/resq-perf

USER resq
ENTRYPOINT ["resq"]
CMD ["--help"]
