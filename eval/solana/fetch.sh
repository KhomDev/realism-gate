#!/usr/bin/env bash
# Fetches the pinned public Rust/Anchor TypeScript test suites used by the
# Solana precision measurement. Network is used here only; nothing after the
# checkouts exist touches the network.
#
# Usage:
#   eval/solana/fetch.sh            fetch and verify every corpus
#   eval/solana/fetch.sh --check    verify the checkouts already present
#
# The checkouts land under .corpus/solana/<name>/, which .gitignore covers.
# Only pointers, hashes and labels are committed; no third-party source is.

set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
corpus="$root/eval/solana/corpus.json"
mode="${1:-fetch}"

if [ "$mode" != "fetch" ] && [ "$mode" != "--check" ]; then
    echo "usage: eval/solana/fetch.sh [--check]" >&2
    exit 2
fi

# Emit: name<TAB>repo<TAB>commit<TAB>dir<TAB>paths-json<TAB>listing-sha256
read_pins() {
    python - "$corpus" <<'PY'
import json,sys
d=json.load(open(sys.argv[1],encoding="utf-8"))
for name,c in d["corpora"].items():
    print("\t".join([name,c["repo"],c["commit"],c["dir"],json.dumps(c["paths"]),c["listing_sha256"]]))
PY
}

while IFS=$'\t' read -r name repo commit dir paths_json want; do
    # Python text mode on Windows ends the line with CRLF; `read` keeps the CR
    # in the last field. Strip it before comparing.
    want="${want%$'\r'}"
    abs="$root/$dir"
    if [ "$mode" = "fetch" ]; then
        if [ ! -d "$abs/.git" ]; then
            mkdir -p "$abs"
            git -C "$abs" init -q
            git -C "$abs" remote add origin "https://github.com/$repo"
            if [ "$name" = "solana-program-library" ]; then
                mapfile -t sparse_paths < <(python - "$paths_json" <<'PY'
import json,sys
for p in json.loads(sys.argv[1]):
    print(p)
PY
                )
                git -C "$abs" sparse-checkout set --no-cone "${sparse_paths[@]}"
            fi
        fi
        have="$(git -C "$abs" rev-parse -q --verify HEAD 2>/dev/null || true)"
        if [ "$have" != "$commit" ]; then
            git -C "$abs" fetch -q --depth 1 --filter=blob:none origin "$commit"
            git -C "$abs" checkout -q --detach FETCH_HEAD
        fi
    fi
    if [ ! -d "$abs/.git" ]; then
        echo "missing checkout: $abs (run without --check)" >&2
        exit 1
    fi
    got="$(git -C "$abs" rev-parse HEAD)"
    if [ "$got" != "$commit" ]; then
        echo "commit mismatch for $name: pinned $commit, found $got" >&2
        exit 1
    fi
    actual="$(python - "$abs" "$commit" "$paths_json" <<'PY'
import hashlib,json,subprocess,sys
d,commit,paths=sys.argv[1],sys.argv[2],json.loads(sys.argv[3])
out=subprocess.run(["git","-C",d,"ls-tree","-r",commit,"--",*paths],check=True,capture_output=True).stdout
print(hashlib.sha256(out).hexdigest())
PY
)"
    actual="${actual%$'\r'}"
    if [ "$actual" != "$want" ]; then
        echo "listing mismatch for $name: pinned $want, found $actual" >&2
        exit 1
    fi
    echo "$name: $commit ($(git -C "$abs" ls-tree -r "$commit" -- 2>/dev/null | wc -l) entries) ok"
done < <(read_pins)
