#!/usr/bin/env bash
# package-order.sh - the local, unverified packaging proof, in dependency order.
#
# This is not a publish. Verification (`cargo package -p <crate>` without
# --no-verify, and `cargo publish --dry-run`) requires the path dependencies to
# be available on crates.io, which they are not until the sub-crates are
# published (an approval-gated release step). See PACKAGING.md.
#
# Usage:
#   scripts/package-order.sh            # package every crate with --no-verify
#   scripts/package-order.sh rg-rules   # one crate
set -euo pipefail
cd "$(dirname "$0")/.."

crates=(
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

if [ "$#" -gt 0 ]; then
  crates=("$@")
fi

for crate in "${crates[@]}"; do
  echo "cargo package -p $crate --no-verify --locked"
  cargo package -p "$crate" --no-verify --locked
done
