# syntax=docker/dockerfile:1.7
#
# Multi-stage build: static musl binary -> distroless, rootless runtime image.
# The build and runtime versions are architecturally separate: the builder is a full
# Debian-based Rust toolchain, but nothing from it (shell, package manager, libc headers)
# reaches the final image — only the compiled static binary does.

FROM rust:1.82-bookworm AS builder
WORKDIR /build

RUN rustup target add x86_64-unknown-linux-musl \
    && apt-get update \
    && apt-get install -y --no-install-recommends musl-tools \
    && rm -rf /var/lib/apt/lists/*

# Cache dependency compilation separately from source changes.
COPY Cargo.toml Cargo.lock ./
COPY crates/meridian-core/Cargo.toml crates/meridian-core/Cargo.toml
RUN mkdir -p crates/meridian-core/src \
    && echo "fn main() {}" > crates/meridian-core/src/main.rs \
    && echo "" > crates/meridian-core/src/lib.rs \
    && cargo build --release --target x86_64-unknown-linux-musl -p meridian-core \
    && rm -rf crates/meridian-core/src

COPY crates/meridian-core ./crates/meridian-core
RUN touch crates/meridian-core/src/main.rs crates/meridian-core/src/lib.rs \
    && cargo build --release --target x86_64-unknown-linux-musl -p meridian-core

# distroless "nonroot" images already run as an unprivileged, non-root UID/GID (65532) with
# no shell and no package manager (INV-010-adjacent posture: minimize what a compromised
# process can reach, applied to the container itself rather than the WASM sandbox).
FROM gcr.io/distroless/static-debian12:nonroot AS runtime

COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/meridian-core /app/meridian-core

USER nonroot:nonroot
ENTRYPOINT ["/app/meridian-core"]
