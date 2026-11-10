#!/usr/bin/env bash
# Prints the name of every binary declared in Cargo.toml, one per line.
#
# The release workflow packages exactly these, and scripts/install-check.sh
# asserts that an archive carries all of them. cargo-binstall installs every
# binary a crate declares and aborts the download when the archive lacks one
# ("bin realism-gate-mcp is not found"), which is what broke the 0.6.0 release.
set -euo pipefail
cd "$(dirname "$0")/.."
awk '/^\[\[bin\]\]/{f=1; next} /^\[/{f=0} f && /^name = "/{gsub(/^name = "|"$/, ""); print}' Cargo.toml
