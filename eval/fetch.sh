#!/usr/bin/env bash
# Fetches every corpus pinned in eval/corpus.json into .corpus/ (gitignored),
# then verifies each one against its pinned listing hash. Only each corpus's
# test directory is checked out. Nothing is vendored.

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
if python3 -c '' 2>/dev/null; then py=python3; else py=python; fi
field() { "$py" "$root/eval/corpus.py" --field "$1"; }

for name in $("$py" "$root/eval/corpus.py" --names | tr -d '\r'); do
    dir="$root/$(field "$name.dir" | tr -d '\r')"
    commit="$(field "$name.commit" | tr -d '\r')"
    repo="$(field "$name.repository" | tr -d '\r')"
    path="$(field "$name.path" | tr -d '\r')"
    if [ ! -d "$dir/.git" ]; then
        mkdir -p "$dir"
        git -C "$dir" init -q
        git -C "$dir" remote add origin "$repo"
        # No leading slash: Git for Windows' shell would rewrite one into a path.
        git -C "$dir" sparse-checkout set --no-cone "$path/"
    fi
    if [ "$(git -C "$dir" rev-parse -q --verify HEAD 2>/dev/null || true)" != "$commit" ]; then
        git -C "$dir" fetch -q --depth 1 --filter=blob:none origin "$commit"
        git -C "$dir" checkout -q --detach FETCH_HEAD
    fi
    test "$(git -C "$dir" rev-parse HEAD)" = "$commit"
done
"$py" "$root/eval/corpus.py" --check
