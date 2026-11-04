#!/usr/bin/env bash
# For action-selftest.yml: checks the files the action just wrote.
#   selftest-check.sh TEXT                  summary.md contains TEXT
#   selftest-check.sh TEXT FIELD VALUE      ... and report.json summary.FIELD == VALUE
set -eu
out="$RUNNER_TEMP/realism-gate-out"
cat "$out/summary.md"
grep -q -- "$1" "$out/summary.md"
if [ $# -ge 3 ]; then
    got=$(grep -m1 "\"$2\":" "$out/report.json" | tr -dc '0-9')
    test "$got" = "$3"
fi
echo "ok: $*"
