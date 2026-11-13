#!/bin/sh
# Build the target tree that examples/rain-usdr cites into a throwaway git
# repo, and print the two values a rerun needs.
#
#   sh examples/rain-usdr/make-target-repo.sh
#   # TARGET examples/rain-usdr/.target-repo
#   # COMMIT <sha>
#
# Then:
#
#   cd examples/rain-usdr
#   realism-gate check test --report finding.md \
#     --target .target-repo --target-commit <sha>
#
# The repo is built at a fixed relative path with a fixed identity and fixed
# dates, so the commit object is the same on every run (same tree, same author,
# same message). Git does not hash the working directory, so the commit id does
# not depend on where this checkout lives. .target-repo/ is gitignored.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
dir="$here/.target-repo"

rm -rf "$dir"
mkdir -p "$dir"
cp -R "$here/target/." "$dir/"

cd "$dir"
git init -q
git config user.name "realism-gate fixture"
git config user.email "fixture@example.invalid"
git add -A
GIT_AUTHOR_DATE="2026-11-12T00:00:00+00:00" \
GIT_COMMITTER_DATE="2026-11-12T00:00:00+00:00" \
  git commit -q -m "RAIN-USDR target tree (fixture)"

echo "TARGET examples/rain-usdr/.target-repo"
echo "COMMIT $(git rev-parse HEAD)"
