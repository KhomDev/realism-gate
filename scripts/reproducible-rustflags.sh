#!/usr/bin/env bash
# Prints the RUSTFLAGS both the release build and the reproduce script use, so
# a published binary does not embed the builder's absolute paths.
#
# Usage: scripts/reproducible-rustflags.sh WORKSPACE
#
# The two builds run in different directories with different cargo homes; each
# path is rewritten to one fixed location:
#   <workspace>            -> /build/realism-gate
#   $CARGO_HOME/registry   -> /cargo/registry
#   $CARGO_HOME/git        -> /cargo/git
#
# Keep this canonical and identical on both sides: a different mapping is a
# different build. Only the Linux releases use it (the Windows and macOS
# binaries do not take part in the archive comparison), so POSIX paths are the
# input.
set -euo pipefail

workspace="${1:-$PWD}"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"

printf '%s\n' \
  "--remap-path-prefix=$workspace=/build/realism-gate" \
  "--remap-path-prefix=$cargo_home/registry=/cargo/registry" \
  "--remap-path-prefix=$cargo_home/git=/cargo/git"
