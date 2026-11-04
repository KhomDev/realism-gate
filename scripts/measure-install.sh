#!/usr/bin/env bash
# Times installing a published realism-gate release without a Rust toolchain,
# and prints one JSON object.
#
#   scripts/measure-install.sh prebuilt TAG TARGET   download the archive and
#                                                    SHA256SUMS, verify, unpack
#   scripts/measure-install.sh binstall TAG          cargo binstall --version
#
# Both run in a fresh temporary directory and finish with `--version`, which
# must match the tag. Network-dependent: record the host and compare like with
# like.

set -euo pipefail

mode="${1:?prebuilt|binstall}"
tag="${2:?tag, e.g. v0.2.0}"
version="${tag#v}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
base="https://github.com/KhomDev/realism-gate/releases/download/$tag"

now_ms() { python3 -c 'import time; print(int(time.time()*1000))' 2>/dev/null || python -c 'import time; print(int(time.time()*1000))'; }

start="$(now_ms)"
case "$mode" in
    prebuilt)
        target="${3:?target triple}"
        case "$target" in *windows*) ext=zip; exe=.exe ;; *) ext=tar.gz; exe= ;; esac
        name="realism-gate-$version-$target"
        (
            cd "$work"
            curl -fsSLO "$base/$name.$ext"
            curl -fsSLO "$base/SHA256SUMS"
            grep " ./$name.$ext\$" SHA256SUMS | sed 's# \./# #' > check.txt
            test -s check.txt
            if command -v sha256sum >/dev/null 2>&1; then sha256sum -c check.txt >/dev/null; else shasum -a 256 -c check.txt >/dev/null; fi
            if [ "$ext" = zip ]; then unzip -q "$name.zip" -d "$name"; else tar xzf "$name.tar.gz"; fi
        )
        bin="$work/$name/realism-gate$exe"
        ;;
    binstall)
        target="$(rustc -vV 2>/dev/null | sed -n 's/^host: //p' || true)"
        cargo binstall realism-gate --version "$version" --no-confirm --root "$work" >/dev/null 2>&1
        bin="$(ls "$work"/bin/realism-gate* | head -n1)"
        ;;
    *)
        echo "unknown mode: $mode" >&2
        exit 2
        ;;
esac
got="$("$bin" --version)"
end="$(now_ms)"
test "$got" = "realism-gate $version"
printf '{"mode": "%s", "tag": "%s", "target": "%s", "seconds": %s, "host": "%s"}\n' \
    "$mode" "$tag" "${target:-}" "$(awk "BEGIN { printf \"%.1f\", ($end - $start) / 1000 }")" "$(uname -sm)"
