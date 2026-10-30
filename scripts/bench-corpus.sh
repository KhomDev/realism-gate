#!/usr/bin/env bash
# Measures realism-gate against a public corpus of reproduced exploits at a
# pinned commit, and compares the result with bench/defihacklabs.json.
#
# The corpus is DeFiHackLabs (Apache-2.0). It is fetched, never vendored: only
# its src/test tree is checked out, under .corpus/, which is gitignored.
#
# Usage:
#   scripts/bench-corpus.sh            print the summary
#   scripts/bench-corpus.sh --check    fail if it differs from bench/defihacklabs.json
#   scripts/bench-corpus.sh --write    overwrite bench/defihacklabs.json
#
# Environment:
#   REALISM_GATE_BIN   binary to measure (default: target/release/realism-gate)

set -euo pipefail

REPO="https://github.com/SunWeb3Sec/DeFiHackLabs"
COMMIT="2c99b565ae24ea2006adf181da20c4419b3edc30"

root="$(cd "$(dirname "$0")/.." && pwd)"
mode="${1:-print}"
bin="${REALISM_GATE_BIN:-$root/target/release/realism-gate}"
corpus="$root/.corpus/defihacklabs"
expected="$root/bench/defihacklabs.json"

if [ ! -x "$bin" ] && [ ! -x "$bin.exe" ]; then
    echo "no binary at $bin; run: cargo build --release" >&2
    exit 2
fi

if [ ! -d "$corpus/.git" ]; then
    mkdir -p "$corpus"
    git -C "$corpus" init -q
    git -C "$corpus" remote add origin "$REPO"
    # No leading slash: Git for Windows' shell would rewrite one into a path.
    git -C "$corpus" sparse-checkout set --no-cone 'src/test/'
fi
if [ "$(git -C "$corpus" rev-parse -q --verify HEAD 2>/dev/null || true)" != "$COMMIT" ]; then
    git -C "$corpus" fetch -q --depth 1 --filter=blob:none origin "$COMMIT"
    git -C "$corpus" checkout -q --detach FETCH_HEAD
fi
test "$(git -C "$corpus" rev-parse HEAD)" = "$COMMIT"

report="$root/.corpus/report.json"
summary="$root/.corpus/summary.json"
(cd "$corpus" && "$bin" check src/test --no-declaration --format json --fail-on never) > "$report"
if python3 -c '' 2>/dev/null; then py=python3; else py=python; fi
"$py" "$root/scripts/summarize_corpus.py" "$report" "$COMMIT" > "$summary"

case "$mode" in
    print)
        cat "$summary"
        ;;
    --write)
        mkdir -p "$(dirname "$expected")"
        cp "$summary" "$expected"
        echo "wrote $expected"
        ;;
    --check)
        if ! diff -u "$expected" "$summary"; then
            echo "corpus numbers moved; if the change is deliberate, run scripts/bench-corpus.sh --write and update docs/benchmark.md" >&2
            exit 1
        fi
        echo "corpus numbers match bench/defihacklabs.json"
        ;;
    *)
        echo "unknown mode: $mode" >&2
        exit 2
        ;;
esac
