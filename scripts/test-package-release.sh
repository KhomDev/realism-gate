#!/usr/bin/env bash
# Local test for scripts/package-release.sh: the same inputs must produce the
# same archive bytes, and a changed input must produce different ones.
#
# The release workflow and scripts/reproduce-release.sh both package through
# that script, so this is the property that makes their archive comparison
# meaningful. Needs GNU tar and sha256sum; the zip half also needs zip and
# unzip and is skipped when they are absent.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
script="$repo/scripts/package-release.sh"
version=1.0.4
target=x86_64-unknown-linux-musl

if ! tar --version 2>/dev/null | grep -q 'GNU tar'; then
  echo "SKIP: package-release determinism needs GNU tar; this host has $(tar --version 2>/dev/null | head -n1)"
  exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
bins="$(bash "$repo/scripts/declared-bins.sh")"

stage() {
  # stage DIR EPOCH: three identical fake binaries with one mtime.
  local dir="$1" epoch="$2" bin
  mkdir -p "$dir"
  for bin in $bins; do
    printf 'same bytes for %s\n' "$bin" > "$dir/$bin"
    chmod 755 "$dir/$bin"
  done
  find "$dir" -exec touch -d "@$epoch" {} +
}

hash_of() { sha256sum "$1" | cut -d' ' -f1; }

fail=0
ok() { printf 'ok    %s\n' "$1"; }
bad() { printf 'FAIL  %s\n' "$1"; fail=1; }

stage "$work/a" 1000000000
stage "$work/b" 1600000000

bash "$script" assemble "$version" "$target" tar.gz "$work/a" "$work/out-a" >/dev/null
bash "$script" assemble "$version" "$target" tar.gz "$work/b" "$work/out-b" >/dev/null
a="$(hash_of "$work/out-a/realism-gate-$version-$target.tar.gz")"
b="$(hash_of "$work/out-b/realism-gate-$version-$target.tar.gz")"
if [ "$a" = "$b" ]; then
  ok "tar.gz is identical across staging paths and mtimes"
else
  bad "tar.gz differs across staging paths and mtimes ($a vs $b)"
fi

# Ownership and mtime are normalized, not inherited from the host.
listing="$(tar tvzf "$work/out-a/realism-gate-$version-$target.tar.gz")"
if printf '%s\n' "$listing" | grep -q ' 1980-01-01 ' && printf '%s\n' "$listing" | grep -q ' 0/0 '; then
  ok "tar.gz entries carry the fixed mtime and uid/gid 0"
else
  bad "tar.gz entries do not carry the fixed mtime and uid/gid 0"
fi

# Control: the check must be able to fail.
printf 'different bytes\n' >> "$work/b/rgate"
bash "$script" assemble "$version" "$target" tar.gz "$work/b" "$work/out-c" >/dev/null
c="$(hash_of "$work/out-c/realism-gate-$version-$target.tar.gz")"
if [ "$a" != "$c" ]; then
  ok "a changed binary changes the tar.gz hash"
else
  bad "a changed binary left the tar.gz hash unchanged (vacuous check)"
fi

if command -v zip >/dev/null 2>&1 && command -v unzip >/dev/null 2>&1; then
  stage "$work/za" 1000000000
  stage "$work/zb" 1600000000
  bash "$script" assemble "$version" x86_64-pc-windows-msvc zip "$work/za" "$work/zout-a" >/dev/null
  bash "$script" assemble "$version" x86_64-pc-windows-msvc zip "$work/zb" "$work/zout-b" >/dev/null
  za="$(hash_of "$work/zout-a/realism-gate-$version-x86_64-pc-windows-msvc.zip")"
  zb="$(hash_of "$work/zout-b/realism-gate-$version-x86_64-pc-windows-msvc.zip")"
  if [ "$za" = "$zb" ]; then
    ok "zip is identical across staging paths and mtimes"
  else
    bad "zip differs across staging paths and mtimes ($za vs $zb)"
  fi
else
  echo "SKIP: zip determinism (zip and unzip are not both on PATH)"
fi

if [ "$fail" -ne 0 ]; then
  echo "package-release test: FAILED"
  exit 1
fi
echo "package-release test: all cases passed"
