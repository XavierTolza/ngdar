#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────
# OpenHands automatic setup script for ngdar
# ──────────────────────────────────────────────

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR"

echo "=== [1/3] Git maintenance ==="

git fetch --all --prune
git remote prune origin

# If on a tracking branch, rebase onto its upstream
CURRENT_BRANCH=$(git symbolic-ref --short HEAD 2>/dev/null || true)
UPSTREAM=$(git rev-parse --abbrev-ref "$CURRENT_BRANCH"@{upstream} 2>/dev/null || true)
if [ -n "$UPSTREAM" ]; then
    echo "Rebasing $CURRENT_BRANCH onto $UPSTREAM …"
    git pull --rebase
fi

echo "=== [2/3] pre-commit setup ==="

# Ensure ~/.local/bin is on PATH (pip installs there in many environments)
export PATH="$HOME/.local/bin:$PATH"

if command -v pre-commit &>/dev/null; then
    echo "pre-commit already available: $(pre-commit --version)"
else
    echo "pre-commit not found – installing via pip …"
    pip install pre-commit 2>&1 | tail -1
fi

echo "=== [3/3] Installing pre-commit hooks ==="

pre-commit install

echo ""
echo "✔ Setup complete."
