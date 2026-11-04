#!/usr/bin/env bash
# Reproduces every precision number published for realism-gate v0.2.0.
#
#   eval/run.sh BEFORE_BIN AFTER_BIN           regenerate eval/flags and eval/results
#   eval/run.sh BEFORE_BIN AFTER_BIN --check   fail if anything differs from the
#                                              committed files
#
# BEFORE_BIN is the realism-gate 0.1.0 release binary, AFTER_BIN a build of this
# tree. The labels (eval/labels/v0.2.0.jsonl) and the samples (eval/samples/)
# are inputs: they are written by hand and by eval/sample.py, not here.
# Nothing here needs the network except eval/fetch.sh's first run.

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
before="${1:?path to the 0.1.0 binary}"
after="${2:?path to the binary under test}"
mode="${3:-write}"
if python3 -c '' 2>/dev/null; then py=python3; else py=python; fi

bash "$root/eval/fetch.sh"

out="$root/eval"
if [ "$mode" = "--check" ]; then
    out="$(mktemp -d)"
    trap 'rm -rf "$out"' EXIT
    mkdir -p "$out/flags" "$out/results"
fi
mkdir -p "$out/flags" "$out/results"

for name in $("$py" "$root/eval/corpus.py" --names | tr -d '\r'); do
    "$py" "$root/eval/flags.py" "$before" "$name" "$out/flags/v0.1.0-$name.jsonl"
    "$py" "$root/eval/flags.py" "$after" "$name" "$out/flags/v0.2.0-$name.jsonl"
done

"$after" rules --format json > "$out/results/rules-v0.2.0.json"

if [ "$mode" = "--check" ]; then
    # precision.py reads the committed flags; point it at the fresh ones.
    for f in "$out"/flags/*.jsonl; do
        diff -u --strip-trailing-cr "$root/eval/flags/$(basename "$f")" "$f"
    done
fi
"$py" "$root/eval/precision.py" "$out/results/precision.json" "$out/results/precision.md" \
    --rules "$out/results/rules-v0.2.0.json"

if [ "$mode" = "--check" ]; then
    for f in precision.json precision.md rules-v0.2.0.json; do
        diff -u --strip-trailing-cr "$root/eval/results/$f" "$out/results/$f"
    done
    echo "eval reproduces: flags, precision and the R3 check match the committed files"
fi
