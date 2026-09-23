#!/usr/bin/env bash
# Run every release gate; the official emulator must be a clean pinned checkout.
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
UPSTREAM="${1:-${EMULSIV_UPSTREAM:-$ROOT/../.emulsiv-upstream}}"
cd "$ROOT"
mkdir -p dist
{
    printf 'SOURCE_COMMIT=%s\n' "$(git rev-parse HEAD)"
    rustc --version
    cargo --version
    python3 --version
    node --version
    cargo test --locked --lib --tests
    cargo test --locked --features format --lib --tests
    python3 -m unittest discover -s tools -p 'test_*.py' -v
    python3 tools/check_runtime.py
    python3 tools/build.py --all
    python3 tools/smoke.py
    node tools/upstream_smoke.mjs "$UPSTREAM"
    node tools/verify_upstream.mjs "$UPSTREAM"
    git diff --check
    printf 'ALL_RELEASE_GATES_PASSED\n'
} 2>&1 | tee dist/verification.log
