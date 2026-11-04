#!/usr/bin/env bash
# Installs realism-gate the way a user would and checks that it works: the
# version matches, and one fixture gives the expected finding and exit code.
#
#   install-check.sh archive VERSION DIR       archive + SHA256SUMS from DIR
#   install-check.sh release VERSION           the same, downloaded from the
#                                              GitHub release vVERSION
#   install-check.sh binstall VERSION          cargo binstall from crates.io,
#                                              prebuilt archives only
#   install-check.sh binstall-mirror VERSION MIRROR
#                                              cargo binstall through this
#                                              checkout's Cargo.toml metadata,
#                                              with the repository URL pointed
#                                              at a local HTTP server serving
#                                              MIRROR (the release layout)
#
# Run from the repository root (it reads tests/fixtures). Prints the seconds
# each install took.

set -euo pipefail

mode="${1:?archive|release|binstall|binstall-mirror}"
version="${2:?version, e.g. 0.2.0}"
version="${version#v}"
root="$(pwd)"
work="$(mktemp -d)"
server=""
cleanup() {
    if [ -n "$server" ]; then kill "$server" 2>/dev/null || true; fi
    rm -rf "$work"
}
trap cleanup EXIT
if python3 -c '' 2>/dev/null; then py=python3; else py=python; fi

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) target=x86_64-unknown-linux-musl ;;
    Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-musl ;;
    Darwin-x86_64) target=x86_64-apple-darwin ;;
    Darwin-arm64) target=aarch64-apple-darwin ;;
    MINGW* | MSYS* | CYGWIN*) target=x86_64-pc-windows-msvc ;;
    *) echo "no build for $(uname -s)-$(uname -m)" >&2; exit 2 ;;
esac
case "$target" in *windows*) ext=zip; exe=.exe ;; *) ext=tar.gz; exe= ;; esac
name="realism-gate-$version-$target"

now() { "$py" -c 'import time; print(time.time())'; }
start="$(now)"

from_archive() {
    local dir="$1"
    (
        cd "$work"
        cp "$dir/$name.$ext" "$dir/SHA256SUMS" .
        grep " ./$name.$ext\$" SHA256SUMS | sed 's# \./# #' > check.txt
        test -s check.txt
        if command -v sha256sum >/dev/null 2>&1; then sha256sum -c check.txt; else shasum -a 256 -c check.txt; fi
        if [ "$ext" = zip ]; then unzip -q "$name.zip"; else tar xzf "$name.tar.gz"; fi
    )
    bin="$work/$name/realism-gate$exe"
}

case "$mode" in
    archive)
        dir="$(cd "${3:?directory with the archives}" && pwd)"
        from_archive "$dir"
        ;;
    release)
        mkdir -p "$work/dl"
        base="https://github.com/KhomDev/realism-gate/releases/download/v$version"
        curl -fsSL -o "$work/dl/$name.$ext" "$base/$name.$ext"
        curl -fsSL -o "$work/dl/SHA256SUMS" "$base/SHA256SUMS"
        from_archive "$work/dl"
        ;;
    binstall)
        # crate-meta-data only: a fallback to compiling from source, or to a
        # third-party binary cache, would hide a broken release.
        cargo binstall realism-gate --version "$version" --no-confirm --force \
            --strategies crate-meta-data --root "$work/root"
        bin="$work/root/bin/realism-gate$exe"
        ;;
    binstall-mirror)
        mirror="$(cd "${3:?mirror root}" && pwd)"
        port=18765
        "$py" -m http.server "$port" --bind 127.0.0.1 --directory "$mirror" >/dev/null 2>&1 &
        server=$!
        for _ in $(seq 1 50); do
            curl -fsS -o /dev/null "http://127.0.0.1:$port/" && break
            sleep 0.2
        done
        mkdir -p "$work/crate"
        cp -r "$root/src" "$work/crate/"
        sed "s#^repository = .*#repository = \"http://127.0.0.1:$port/KhomDev/realism-gate\"#" \
            "$root/Cargo.toml" > "$work/crate/Cargo.toml"
        grep -q "^version = \"$version\"" "$work/crate/Cargo.toml"
        cargo binstall --manifest-path "$work/crate/Cargo.toml" realism-gate --no-confirm --force \
            --strategies crate-meta-data --no-discover-github-token --root "$work/root"
        bin="$work/root/bin/realism-gate$exe"
        ;;
    *)
        echo "unknown mode: $mode" >&2
        exit 2
        ;;
esac
end="$(now)"

got="$("$bin" --version | tr -d '\r')"
if [ "$got" != "realism-gate $version" ]; then
    echo "::error::expected 'realism-gate $version', got '$got'"
    exit 1
fi

set +e
"$bin" check "$root/tests/fixtures/bad/RG06_balance_injection.t.sol" \
    --no-declaration --no-config --format json > "$work/report.json"
status=$?
set -e
if [ "$status" != 1 ]; then
    echo "::error::the RG06 fixture must exit 1, got $status"
    exit 1
fi
"$py" - "$work/report.json" <<'EOF'
import json, sys
r = json.load(open(sys.argv[1], encoding="utf-8"))
got = [(f["rule"], f["severity"]) for f in r["findings"]]
assert got == [("RG06", "error")], got
EOF
"$py" -c "print('ok  $mode  $target  ' + format($end - $start, '.1f') + ' s  ' + '$got')"
