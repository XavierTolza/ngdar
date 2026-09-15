#!/usr/bin/env bash
#
# Verify that a standalone ngdar binary really runs in a bare environment.
#
# The binary is dropped into a Docker image built `FROM scratch`: a completely
# empty filesystem with no libc, no shell, no tools. If ngdar has any runtime
# dependency (shared library, dynamic loader, …), it fails here. We then drive
# a full init -> add -> pack -> log workflow, each command running as a fresh
# container against a shared working directory, to prove the binary is
# functional, not merely loadable.
#
# Usage:
#   scripts/verify-standalone.sh <path-to-binary>
#
# Requires: docker.

set -euo pipefail

BIN="${1:?usage: verify-standalone.sh <path-to-binary>}"

if [ ! -f "$BIN" ]; then
  echo "error: binary not found at $BIN" >&2
  exit 1
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "error: docker is required to verify the standalone binary" >&2
  exit 1
fi

BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"

WORK="$(mktemp -d)"
# Shared scratch dir: the workspace the containers operate on. Created on the
# host because the scratch image has no tools of its own.
DATA="$(mktemp -d)"
IMAGE="ngdar-standalone-verify:$$"
cleanup() {
  docker image rm -f "$IMAGE" >/dev/null 2>&1 || true
  rm -rf "$WORK" "$DATA"
}
trap cleanup EXIT

# The image contains exactly one file: the binary. No shell to fall back on,
# so anything it needs at runtime must already be compiled into it.
cp "$BIN" "$WORK/ngdar"
cat > "$WORK/Dockerfile" <<'EOF'
FROM scratch
COPY ngdar /ngdar
ENTRYPOINT ["/ngdar"]
EOF

docker build -q -t "$IMAGE" "$WORK" >/dev/null

# Create the input file on the host: the scratch container has no `echo`.
printf 'ngdar standalone verification\n' > "$DATA/file.txt"

run() {
  docker run --rm -v "$DATA:/work" -w /work "$IMAGE" "$@"
}

echo "==> ngdar --version in an empty (scratch) container"
run --version

echo "==> full init -> add -> pack -> log workflow in the same image"
run init
test -d "$DATA/.ngdar" || { echo "error: init did not create .ngdar" >&2; exit 1; }

run add file.txt
run pack --vol-id VERIFY-001 --out /work/verify.tar -m "standalone verification"
test -f "$DATA/verify.tar" || { echo "error: pack did not create the archive" >&2; exit 1; }

run log
run status

echo "OK: the binary runs standalone in an empty environment."
