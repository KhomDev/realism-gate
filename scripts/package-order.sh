#!/usr/bin/env bash
# package-order.sh - produce every .crate archive, in dependency order.
#
# This is not a publish. Plain `cargo package -p realism-gate --no-verify`
# fails on cargo 1.95.0 with "no matching package named `rg-exec` found":
# Cargo rewrites path dependencies to their crates.io versions and resolves
# them before it writes the archive, and the sub-crates are publish = false.
# Only rg-ir has no workspace dependency and packages plainly.
#
# To rehearse the packaging locally, this script passes a temporary
# [patch.crates-io] section that maps each workspace crate to its path. The
# archives are the ones the registry would receive; publishing them in this
# order remains an approval-gated outward action. See PACKAGING.md.
#
# Usage:
#   scripts/package-order.sh              # package every crate
#   scripts/package-order.sh rg-rules     # one crate (patches all the others)
set -euo pipefail
cd "$(dirname "$0")/.."

root="$(pwd)"
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) root="$(cygpath -m "$root")" ;;
esac

all_crates=(
  rg-ir
  rg-frontend-sol
  rg-frontend-rust
  rg-frontend-ts
  rg-frontend-move
  rg-program
  rg-model
  rg-rules
  rg-exec
  rg-render
  realism-gate
)

crates=("$@")
if [ "${#crates[@]}" -eq 0 ]; then
  crates=("${all_crates[@]}")
fi

patch_file="$(mktemp)"
trap 'rm -f "$patch_file"' EXIT
{
  echo "[patch.crates-io]"
  for crate in "${all_crates[@]}"; do
    [ "$crate" = "realism-gate" ] && continue
    echo "$crate = { path = \"$root/crates/$crate\" }"
  done
} > "$patch_file"

for crate in "${crates[@]}"; do
  echo "cargo package -p $crate --no-verify --locked --config <local patch>"
  cargo package -p "$crate" --no-verify --locked --config "$patch_file"
done
