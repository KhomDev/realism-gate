#!/usr/bin/env bash
# Rebuild a published release from its source archive and compare it with the
# release's own bytes.
#
# Usage: scripts/reproduce-release.sh [--binary-only] TAG [TARGET]
#
#   TAG     a published tag, e.g. v1.0.5
#   TARGET  default x86_64-unknown-linux-musl; must be a target the release
#           carries an archive for
#
# Environment: RG_TOOLCHAIN (default 1.95.0, the release workflow's pin),
# RG_REPO (default KhomDev/realism-gate).
#
# What it does:
#   1. downloads the source archive for the tag, the release archive for
#      TARGET, and the release SHA256SUMS;
#   2. checks the downloaded archive against SHA256SUMS (so the bytes being
#      compared are the published ones);
#   3. builds the tag from the source archive with the pinned toolchain and
#      path-remapping flags from scripts/reproducible-rustflags.sh;
#   4. compares the SHA256 of every declared binary in the released archive
#      with the rebuilt binary;
#   5. repackages the rebuilt binaries with scripts/package-release.sh and
#      compares that archive's SHA256 with the SHA256SUMS entry.
#
# Exit 0 only when the binary hashes and (unless --binary-only) the archive
# hashes all match. A mismatch prints both values and the reason that is known,
# never a pass. Releases built before the deterministic packaging change have
# non-reproducible tar/zip metadata and an unpinned `stable` toolchain; for
# those, expect a mismatch and use --binary-only to record the binary result.
#
# The archive hash comparison assumes the same packaging environment (GNU tar,
# gzip, zip versions) as the release's checksums job; the workflow uses the
# same ubuntu-24.04 runner for that reason.
set -euo pipefail

binary_only=0
if [ "${1:-}" = "--binary-only" ]; then
  binary_only=1
  shift
fi

tag="${1:-}"
target="${2:-x86_64-unknown-linux-musl}"
if [ -z "$tag" ]; then
  echo "usage: $0 [--binary-only] TAG [TARGET]" >&2
  exit 2
fi
repo="${RG_REPO:-KhomDev/realism-gate}"
toolchain="${RG_TOOLCHAIN:-1.95.0}"
version="${tag#v}"
name="realism-gate-$version-$target"

case "$target" in
  *windows*) ext=".exe" ;;
  *) ext="" ;;
esac
case "$target" in
  *-musl|*-linux-gnu) archive=tar.gz ;;
  *windows*) archive=zip ;;
  *) archive=tar.gz ;;
esac

for tool in curl tar gzip sha256sum rustup; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "missing required tool: $tool" >&2
    exit 2
  fi
done
if [ "$archive" = zip ] && ! command -v unzip >/dev/null 2>&1; then
  echo "missing required tool: unzip (needed for the Windows archive)" >&2
  exit 2
fi
if ! rustup run "$toolchain" rustc --version >/dev/null 2>&1; then
  echo "toolchain $toolchain is not installed; run: rustup toolchain install $toolchain --profile minimal" >&2
  exit 2
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
base="https://github.com/$repo/releases/download/$tag"
src_url="https://github.com/$repo/archive/refs/tags/$tag.tar.gz"

echo "release:  $repo $tag"
echo "target:   $target"
echo "toolchain: $toolchain ($(rustup run "$toolchain" rustc --version))"

curl -fsSL --retry 3 -o "$work/source.tar.gz" "$src_url"
curl -fsSL --retry 3 -o "$work/$name.$archive" "$base/$name.$archive"
curl -fsSL --retry 3 -o "$work/SHA256SUMS" "$base/SHA256SUMS"

source_sha="$(sha256sum "$work/source.tar.gz" | cut -d' ' -f1)"
echo "source archive sha256: $source_sha"

# The published archive must match the published checksum before its contents
# are treated as the release.
(
  cd "$work"
  grep " \./$name.$archive\$" SHA256SUMS | sed 's# \./# #' > check.txt
  test -s check.txt
  sha256sum -c check.txt
)
released_archive_sha="$(sha256sum "$work/$name.$archive" | cut -d' ' -f1)"
echo "released archive sha256: $released_archive_sha"

tar xzf "$work/source.tar.gz" -C "$work"
src="$work/realism-gate-$version"
test -f "$src/Cargo.toml"
built_version="$(grep -E '^version' "$src/Cargo.toml" | head -n1 | sed -E 's/.*"(.*)".*/\1/')"
if [ "$built_version" != "$version" ]; then
  echo "source archive Cargo.toml says $built_version, tag is $version" >&2
  exit 1
fi

mkdir -p "$work/released"
if [ "$archive" = zip ]; then
  unzip -q "$work/$name.zip" -d "$work/released"
else
  tar xzf "$work/$name.tar.gz" -C "$work/released"
fi
released_dir="$work/released/$name"
test -d "$released_dir"

(
  cd "$src"
  export RUSTFLAGS="$(bash "$src/scripts/reproducible-rustflags.sh" "$src")"
  rustup run "$toolchain" cargo build --release --locked --target "$target"
)
rebuilt_dir="$src/target/$target/release"

status=0
bins="$(bash "$src/scripts/declared-bins.sh")"
echo
echo "binary comparison (rebuilt vs released):"
for bin in $bins; do
  if [ ! -f "$rebuilt_dir/$bin$ext" ]; then
    echo "  MISSING  $bin$ext: the rebuild did not produce it"
    status=1
    continue
  fi
  new_sha="$(sha256sum "$rebuilt_dir/$bin$ext" | cut -d' ' -f1)"
  rel_sha="$(sha256sum "$released_dir/$bin$ext" | cut -d' ' -f1)"
  if [ "$new_sha" = "$rel_sha" ]; then
    echo "  MATCH    $bin$ext $new_sha"
  else
    echo "  MISMATCH $bin$ext"
    echo "           rebuilt  $new_sha"
    echo "           released $rel_sha"
    status=1
  fi
done

mkdir -p "$work/repacked"
bash "$src/scripts/package-release.sh" assemble "$version" "$target" "$archive" "$rebuilt_dir" "$work/repacked" >/dev/null
if [ "$archive" = tar.gz ]; then
  rebuilt_archive="$work/repacked/$name.tar.gz"
else
  rebuilt_archive="$work/repacked/$name.zip"
fi
rebuilt_archive_sha="$(sha256sum "$rebuilt_archive" | cut -d' ' -f1)"
entry_sha="$(grep " \./$name.$archive\$" "$work/SHA256SUMS" | cut -d' ' -f1)"

echo
echo "archive comparison:"
echo "  released entry   $entry_sha"
echo "  rebuilt archive  $rebuilt_archive_sha"
if [ "$rebuilt_archive_sha" = "$entry_sha" ]; then
  echo "  MATCH    the rebuilt archive is byte-identical to the released entry"
else
  echo "  DIFFERS  the rebuilt archive is not byte-identical"
  if [ "$binary_only" -eq 1 ]; then
    echo "           --binary-only: archive difference is reported, not fatal"
  else
    echo "           (the binary comparison above is still meaningful; the"
    echo "           difference is packaging metadata or the toolchain/environment)"
    status=1
  fi
fi

echo
if [ "$status" -eq 0 ]; then
  echo "reproducible: YES ($repo $tag $target)"
else
  echo "reproducible: NO ($repo $tag $target)"
fi
exit "$status"
