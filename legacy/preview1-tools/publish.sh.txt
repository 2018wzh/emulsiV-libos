#!/usr/bin/env bash
# Publish only this clean, validated repository, without exposing credentials.
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
for tool in gh git cargo python3 node clang; do
    command -v "$tool" >/dev/null || { echo "Required command unavailable: $tool" >&2; exit 1; }
done
[[ "$(git rev-parse --show-toplevel)" == "$ROOT" ]] || {
    echo "Run from the dedicated emulsiV-libos Git checkout (or clone the .bundle first)." >&2; exit 1;
}
[[ "$(git branch --show-current)" == main ]] || { echo "Expected branch main." >&2; exit 1; }
[[ -z "$(git status --porcelain)" ]] || { echo "Commit or review local changes before publication." >&2; exit 1; }
OWNER="$(gh api user --jq .login)"
[[ "$OWNER" == "2018wzh" ]] || { echo "Authenticated account is not the intended owner 2018wzh." >&2; exit 1; }
bash tools/verify.sh
REPO="$OWNER/emulsiV-libos"
if git remote get-url origin >/dev/null 2>&1; then
    REMOTE="$(git remote get-url origin)"
    case "$REMOTE" in
        "https://github.com/$REPO"|"https://github.com/$REPO.git"|"git@github.com:$REPO.git") ;;
        *) echo "Refusing to change or push an unrelated origin: $REMOTE" >&2; exit 1 ;;
    esac
    git push --set-upstream origin main
else
    # Private by default. An existing remote repository causes gh to fail safely.
    gh repo create "$REPO" --private --source="$ROOT" --remote=origin --push \
        --description 'Allocation-free Rust libOS for emulsiV / Virgule'
fi
gh repo view "$REPO" --json url --jq .url
