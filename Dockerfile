# syntax=docker/dockerfile:1
#
# NGDAR — New Generation Disk Archiving
#
# Produces the smallest possible image: a single, fully static musl binary
# dropped into an empty (`scratch`) image. This matches the guarantee made by
# the CI (`.github/workflows/standalone-linux.yml`): the binary has no runtime
# dependencies at all, so the size of a layer is exactly the size of the
# compiled program.
#
# Build:
#   docker build -t ngdar .
# Run (work on a mounted directory):
#   docker run --rm -v "$PWD:/work" ngdar init
#   docker run --rm -v "$PWD:/work" ngdar add mydata/

ARG VERSION=dev
ARG REVISION=unknown
ARG SOURCE_URL=https://github.com/XavierTolza/ngdar

# ─────────────────────────────────────────────
# Builder: compile a fully static musl binary.
# The `rust:*-alpine` image targets musl by default, which links the CRT
# statically — no extra toolchain needed.
# ─────────────────────────────────────────────
FROM rust:1-alpine AS builder

WORKDIR /build

# Prime the dependency layer. Cargo.toml/Cargo.lock rarely change, so this
# layer is cached and rebuilds only compile the crate itself.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release --locked

COPY src ./src
RUN touch src/main.rs && \
    cargo build --release --locked

# ─────────────────────────────────────────────
# Runtime: just the single static binary and nothing else.
# ─────────────────────────────────────────────
FROM scratch

ARG VERSION
ARG REVISION
ARG SOURCE_URL

LABEL org.opencontainers.image.title="ngdar" \
      org.opencontainers.image.description="New Generation Disk Archiving - Git-like incremental archiving for massive binary files" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${REVISION}" \
      org.opencontainers.image.source="${SOURCE_URL}" \
      org.opencontainers.image.licenses="MIT"

COPY --from=builder /build/target/release/ngdar /ngdar

# Nice default working directory for a mounted data volume.
WORKDIR /work

# Build-time smoke test: fail the build if the binary does not run in this
# empty image (e.g. accidentally linked against a libc that `scratch` lacks).
# Exec-form RUN needs no shell, which `scratch` does not have.
RUN ["/ngdar", "--version"]

ENTRYPOINT ["/ngdar"]
CMD ["--help"]
