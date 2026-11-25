#!/usr/bin/env bash
# Fetches the pinned public Solana PoC sources for the P1-C fresh corpus (v2).
# Network is used here only. Sources are fetched one at a time (shallow, to the
# pinned commit); nothing is built or run.
#
# Three kinds of pin, all verified by this script:
#   - git sources        repo + commit + sha256 of `git ls-tree -r <commit>`
#   - issue bodies       the Solana Audit Arena issues, pinned per issue by
#                        sha256 of the markdown body (they live outside git)
#   - report pages       rendered HTML pages, pinned by sha256 of the file
#
# Usage:
#   eval/solana-poc-v2/fetch.sh            fetch and verify every source
#   eval/solana-poc-v2/fetch.sh --check    verify what is present, download nothing
#
# Checkouts land under .corpus/solana-poc-v2/<name>/ (gitignored). Only
# pointers, hashes and labels are committed; no third-party text is.

set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
corpus="$root/eval/solana-poc-v2/corpus.json"
mode="${1:-fetch}"

if [ "$mode" != "fetch" ] && [ "$mode" != "--check" ]; then
    echo "usage: eval/solana-poc-v2/fetch.sh [--check]" >&2
    exit 2
fi

# Emit: name<TAB>repo<TAB>commit<TAB>dir<TAB>paths-json<TAB>listing-sha256
read_pins() {
    python - "$corpus" <<'PY'
import json,sys
d=json.load(open(sys.argv[1],encoding="utf-8"))
for name,c in d["corpora"].items():
    if not c.get("repo"):
        continue
    print("\t".join([name,c["repo"],c["commit"],c["dir"],json.dumps(c["paths"]),c["listing_sha256"] or "unpinned"]))
PY
}

# Emit: number<TAB>sha256
read_issues() {
    python - "$corpus" <<'PY'
import json,sys
d=json.load(open(sys.argv[1],encoding="utf-8"))
for n,p in d["arena_issues"].items():
    print("\t".join([n,p["sha256"]]))
PY
}

# Emit: name<TAB>url<TAB>file<TAB>sha256
read_reports() {
    python - "$corpus" <<'PY'
import json,sys
d=json.load(open(sys.argv[1],encoding="utf-8"))
for name,r in d["reports"].items():
    print("\t".join([name,r.get("url") or "",r["file"],r["sha256"]]))
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

# Arena issues: fetched from the public repository's issue API when missing.
# They are not in git, so the body sha256 is the pin.
while IFS=$'\t' read -r number want; do
    want="${want%$'\r'}"
    file="$root/.corpus/solana-poc-v2/issues/$number.md"
    if [ "$mode" = "fetch" ] && [ ! -s "$file" ]; then
        mkdir -p "$(dirname "$file")"
        gh api "repos/Frankcastleauditor/Solana-Audit-Arena/issues/$number" --jq .body > "$file"
    fi
    if [ ! -s "$file" ]; then
        echo "missing arena issue $number (run without --check)" >&2
        exit 1
    fi
    actual="$(python - "$file" <<'PY'
import hashlib,sys
b=open(sys.argv[1],"rb").read().decode("utf-8","replace")
b=b.replace("\r\n","\n").replace("\r","\n").rstrip()
print(hashlib.sha256(b.encode("utf-8")).hexdigest())
PY
)"
    actual="${actual%$'\r'}"
    if [ "$actual" != "$want" ]; then
        echo "issue body mismatch for $number: pinned $want, found $actual" >&2
        exit 1
    fi
    echo "issue $number ok"
done < <(read_issues)

# Public report pages (rendered HTML), pinned by file sha256.
while IFS=$'\t' read -r name url file want; do
    file="${file%$'\r'}"; want="${want%$'\r'}"
    if [ "$mode" = "fetch" ] && [ ! -s "$root/$file" ] && [ -n "$url" ]; then
        mkdir -p "$(dirname "$root/$file")"
        curl -sSfL -A "Mozilla/5.0" -o "$root/$file" "$url"
    fi
    if [ ! -s "$root/$file" ]; then
        echo "missing report: $file (run without --check)" >&2
        exit 1
    fi
    actual="$(sha256sum "$root/$file" | cut -d' ' -f1)"
    if [ "$actual" != "$want" ]; then
        echo "report hash mismatch for $name: pinned $want, found $actual" >&2
        exit 1
    fi
    echo "report $name ok"
done < <(read_reports)

exit $status
