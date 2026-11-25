#!/usr/bin/env bash
# Fetches the pinned public Solana PoC sources for the PoC precision study.
# Network is used here only. Sources are fetched one at a time (shallow,
# blob-filtered, sparse to the pinned paths); nothing is built or run.
#
# Usage:
#   eval/solana-poc/fetch.sh            fetch and verify every source
#   eval/solana-poc/fetch.sh --check    verify the checkouts already present
#
# Checkouts land under .corpus/solana-poc/<name>/ (gitignored). Only pointers,
# hashes and labels are committed; no third-party text is.

set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
corpus="$root/eval/solana-poc/corpus.json"
mode="${1:-fetch}"

if [ "$mode" != "fetch" ] && [ "$mode" != "--check" ]; then
    echo "usage: eval/solana-poc/fetch.sh [--check]" >&2
    exit 2
fi

# Emit: name<TAB>repo<TAB>commit<TAB>dir<TAB>paths-json<TAB>listing-sha256
read_pins() {
    python - "$corpus" <<'PY'
import json,sys
d=json.load(open(sys.argv[1],encoding="utf-8"))
for name,c in d["corpora"].items():
    print("\t".join([name,c["repo"],c["commit"],c["dir"],json.dumps(c["paths"]),c["listing_sha256"] or "unpinned"]))
PY
}

# Emit: name<TAB>url<TAB>file
read_reports() {
    python - "$corpus" <<'PY'
import json,sys
d=json.load(open(sys.argv[1],encoding="utf-8"))
for name,r in d.get("reports",{}).items():
    print("\t".join([name,r["url"],r["file"]]))
PY
}

status=0
while IFS=$'\t' read -r name repo commit dir paths_json want; do
    want="${want%$'\r'}"
    abs="$root/$dir"
    if [ "$mode" = "fetch" ]; then
        if [ ! -d "$abs/.git" ]; then
            mkdir -p "$abs"
            git -C "$abs" init -q
            git -C "$abs" config core.autocrlf false
            git -C "$abs" remote add origin "https://github.com/$repo"
            # Python text mode on Windows ends each path with CR; strip it,
            # or git sparse-matches nothing.
            mapfile -t sparse_paths < <(python -c 'import json,sys; [print(p) for p in json.loads(sys.argv[1])]' "$paths_json" | tr -d '\r')
            if [ "${sparse_paths[*]}" != "." ]; then
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
        echo "missing checkout: $dir (run without --check)" >&2
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
    if [ "$want" = "unpinned" ]; then
        echo "$name: $commit listing $actual (UNPINNED)"
        status=1
    elif [ "$actual" != "$want" ]; then
        echo "listing mismatch for $name: pinned $want, found $actual" >&2
        exit 1
    else
        echo "$name: $commit ok"
    fi
done < <(read_pins)

# Public report pages (rendered HTML). Not hash-pinned: a page can be
# re-rendered. The PoC blocks extracted from it are pinned in inventory.json,
# and `extract.py` followed by `inventory.py --check` verifies them.
while IFS=$'\t' read -r name url file; do
    file="${file%$'\r'}"
    if [ "$mode" = "fetch" ]; then
        mkdir -p "$(dirname "$root/$file")"
        curl -sSfL -A "Mozilla/5.0" -o "$root/$file" "$url"
    fi
    if [ ! -s "$root/$file" ]; then
        echo "missing report: $file (run without --check)" >&2
        exit 1
    fi
    echo "report $name: $file present"
done < <(read_reports)
exit $status
