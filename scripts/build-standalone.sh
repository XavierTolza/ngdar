#!/usr/bin/env bash
#
# Build a portable, fully standalone ngdar binary for Linux.
#
# The resulting executable is statically linked against musl, so it runs on
# any Linux distribution (or even a `scratch` container) without any runtime
# dependency — no libc, no loader, nothing to install.
#
# Usage:
#   scripts/build-standalone.sh [TARGET] [OUTPUT_DIR]
#
# Defaults:
#   TARGET      x86_64-unknown-linux-musl
#   OUTPUT_DIR  dist
#
# Output:
#   <OUTPUT_DIR>/ngdar-<version>-<target>-static.tar.gz   (binary + README + LICENSE)

set -euo pipefail

TARGET="${1:-x86_64-unknown-linux-musl}"
OUTPUT_DIR="${2:-dist}"

# Resolve the repository root regardless of where the script is called from.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

case "$TARGET" in
  *-unknown-linux-musl) ;;
  *)
    echo "error: only *-unknown-linux-musl targets produce a standalone binary (got '$TARGET')" >&2
    exit 1
    ;;
esac

# Version is the single source of truth in Cargo.toml.
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
if [ -z "$VERSION" ]; then
  echo "error: could not read version from Cargo.toml" >&2
  exit 1
fi

echo "Building ngdar $VERSION for $TARGET (static)…"

if ! rustup target list --installed | grep -qx "$TARGET"; then
  echo "Installing Rust target $TARGET…"
  rustup target add "$TARGET"
fi

# +crt-static guarantees the libc is linked into the binary rather than
# relied upon at runtime. This is what makes the binary portable.
RUSTFLAGS="-C target-feature=+crt-static" \
  cargo build --release --locked --target "$TARGET"

BIN="target/$TARGET/release/ngdar"
if [ ! -f "$BIN" ]; then
  echo "error: expected binary not found at $BIN" >&2
  exit 1
fi

# Fail fast if the binary is not actually static — that is the whole point.
if command -v ldd >/dev/null 2>&1 && ldd "$BIN" 2>&1 | grep -q 'not a dynamic executable\|statically linked'; then
  echo "OK: $BIN is statically linked"
else
  echo "error: $BIN is not statically linked" >&2
  ldd "$BIN" || true
  exit 1
fi

STAGE="ngdar-${VERSION}-${TARGET}-static"
mkdir -p "$OUTPUT_DIR/$STAGE"
cp "$BIN" "$OUTPUT_DIR/$STAGE/ngdar"
cp README.md LICENSE "$OUTPUT_DIR/$STAGE/"
tar -C "$OUTPUT_DIR" -czf "$OUTPUT_DIR/${STAGE}.tar.gz" "$STAGE"
rm -rf "$OUTPUT_DIR/$STAGE"

echo "Created $OUTPUT_DIR/${STAGE}.tar.gz"
