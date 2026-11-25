#!/usr/bin/env bash
# Deterministic packaging for one release target.
#
# Usage:
#   scripts/package-release.sh assemble  VERSION TARGET ARCHIVE BIN_DIR   OUT_DIR
#   scripts/package-release.sh repackage VERSION TARGET ARCHIVE SRC_PARENT OUT_DIR
#
# `assemble` builds the release directory from freshly built binaries and the
# documentation in this checkout. `repackage` normalizes a directory that
# already has the release layout, which is what the release workflow does to
# the archives each build job produced.
#
# Every entry gets the same mtime, uid, gid and sorted order, and gzip runs with
# -n (no timestamp, no file name), so the same inputs produce the same archive
# bytes. That is what makes the released SHA256SUMS entry comparable with a
# rebuild by scripts/reproduce-release.sh. Requires GNU tar (Linux) for
# tar.gz; zip archives are written with `zip -X -@` from a sorted file list.
#
# SOURCE_DATE_EPOCH defaults to 1980-01-01T00:00:00Z, the earliest timestamp a
# zip entry can store. Both the release workflow and the reproduce script use
# the default; override it only on both sides.
set -euo pipefail

mode="${1:-}"
version="${2:-}"
target="${3:-}"
archive="${4:-}"
source="${5:-}"
out_dir="${6:-}"

if [ -z "$mode" ] || [ -z "$version" ] || [ -z "$target" ] || [ -z "$archive" ] || [ -z "$source" ] || [ -z "$out_dir" ]; then
  echo "usage: $0 assemble|repackage VERSION TARGET tar.gz|zip SRC OUT_DIR" >&2
  exit 2
fi
case "$archive" in
  tar.gz|zip) ;;
  *) echo "archive must be tar.gz or zip, not '$archive'" >&2; exit 2 ;;
esac

repo="$(cd "$(dirname "$0")/.." && pwd)"
name="realism-gate-$version-$target"
epoch="${SOURCE_DATE_EPOCH:-315532800}"

if [ "$archive" = tar.gz ] && ! tar --version 2>/dev/null | grep -q 'GNU tar'; then
  echo "package-release.sh needs GNU tar for a byte-reproducible tar.gz; this is $(
    tar --version 2>/dev/null | head -n1) on this host" >&2
  exit 2
fi
if [ "$archive" = zip ] && ! command -v zip >/dev/null 2>&1; then
  echo "package-release.sh needs zip for the Windows archive, and it is not on PATH" >&2
  exit 2
fi

work=""
cleanup() { if [ -n "$work" ]; then rm -rf "$work"; fi; }
trap cleanup EXIT

case "$mode" in
  assemble)
    bin_dir="$(cd "$source" && pwd)"
    work="$(mktemp -d)"
    mkdir -p "$work/$name"
    ext=""
    if [ "$archive" = zip ]; then ext=".exe"; fi
    # Cargo.toml is the list of binaries; never write it out again here.
    bins="$(bash "$repo/scripts/declared-bins.sh")"
    for bin in $bins; do
      test -f "$bin_dir/$bin$ext"
      cp "$bin_dir/$bin$ext" "$work/$name/"
    done
    cp "$repo/README.md" "$repo/LICENSE" "$repo/CHANGELOG.md" "$work/$name/"
    src_parent="$work"
    ;;
  repackage)
    src_parent="$(cd "$source" && pwd)"
    test -d "$src_parent/$name"
    ;;
  *)
    echo "mode must be assemble or repackage, not '$mode'" >&2
    exit 2
    ;;
esac

# One mtime for every entry (zip reads it from the file; tar gets --mtime).
find "$src_parent/$name" -exec touch -d "@$epoch" {} +

mkdir -p "$out_dir"
# Absolute, because the zip branch writes from inside $src_parent.
out_dir="$(cd "$out_dir" && pwd)"
case "$archive" in
  tar.gz)
    # --sort=name: readdir order must not leak into the archive.
    # --format=gnu: do not depend on the host tar's default format.
    LC_ALL=C tar --format=gnu --sort=name --mtime="@$epoch" \
      --owner=0 --group=0 --numeric-owner \
      -C "$src_parent" -cf - "$name" | gzip -n -9 > "$out_dir/$name.tar.gz"
    ;;
  zip)
    # Sorted file list, no extra attributes, no directory entries (unzip
    # creates the directories). -X drops uid/gid and the extra fields.
    (
      cd "$src_parent"
      find "$name" -type f -print | LC_ALL=C sort | zip -X -q "$out_dir/$name.zip" -@
    )
    ;;
esac

echo "$out_dir/$name.$archive"
