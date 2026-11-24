#!/bin/bash -eu
# Build realism-gate's six parser fuzz targets for OSS-Fuzz.
#
# cargo-fuzz builds in $SRC/realism-gate/fuzz (its own workspace), with nightly
# and AddressSanitizer. Every target is a libFuzzer binary; each one's property
# is "this parser never panics on any input". The target names match
# fuzz/fuzz_targets/*.rs and fuzz/corpus/<target>/.

cd "$SRC/realism-gate/fuzz"

# The pinned 1.0.4 commit's fuzz/Cargo.lock still names the root package 1.0.1
# (Cargo.toml says 1.0.4), so `--locked` aborts before compiling there. Cargo
# refreshes exactly that one entry from Cargo.toml and reuses every other
# locked dependency version; the committed lockfile is fixed in the
# trust-infrastructure change for future pins. Do not add --locked until
# RG_COMMIT points at a commit that contains the fix.
cargo metadata --format-version 1 > /dev/null

# -O: optimizations without debug assertions, the libFuzzer default profile in
# OSS-Fuzz. -s address: the one sanitizer project.yaml lists, stated explicitly
# rather than inferred from the base image.
cargo fuzz build -O -s address

out="$SRC/realism-gate/fuzz/target/x86_64-unknown-linux-gnu/release"
targets="solidity rust typescript move claims declaration"
for target in $targets; do
  test -x "$out/$target"
  cp "$out/$target" "$OUT/"
  if [ -d "corpus/$target" ] && [ -n "$(ls -A "corpus/$target")" ]; then
    # -j: files at the zip root, which is the layout libFuzzer reads.
    zip -qjr "$OUT/${target}_seed_corpus.zip" "corpus/$target"
  fi
done
