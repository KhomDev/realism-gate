#!/usr/bin/env python3
"""Reduces a realism-gate JSON report to the counts published in
docs/benchmark.md. Standard library only. Output is deterministic.

Usage: summarize_corpus.py REPORT.json COMMIT
"""

from __future__ import annotations

import json
import sys
from collections import defaultdict


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    with open(argv[1], encoding="utf-8") as fh:
        report = json.load(fh)

    findings = [f for f in report["findings"] if "suppressed" not in f]
    by_rule: dict[str, dict[str, int]] = defaultdict(lambda: defaultdict(int))
    tests_by_rule: dict[str, set[tuple[str, str, str]]] = defaultdict(set)
    flagged: set[tuple[str, str, str]] = set()
    flagged_beyond_rg08: set[tuple[str, str, str]] = set()
    for f in findings:
        by_rule[f["rule"]][f["severity"]] += 1
        for t in f["tests"]:
            ident = (f["file"], f["suite"], t)
            tests_by_rule[f["rule"]].add(ident)
            flagged.add(ident)
            if f["rule"] != "RG08":
                flagged_beyond_rg08.add(ident)

    summary = report["summary"]
    out = {
        "corpus": "DeFiHackLabs",
        "commit": argv[2],
        "files": summary["files"],
        "suites": summary["suites"],
        "tests": summary["tests"],
        "files_with_parse_errors": len({e["file"] for e in report["parse_errors"]}),
        "tests_with_any_finding": len(flagged),
        "tests_with_a_finding_other_than_RG08": len(flagged_beyond_rg08),
        "rules": {
            rule: {
                "findings": dict(sorted(by_rule[rule].items())),
                "tests": len(tests_by_rule[rule]),
            }
            for rule in sorted(by_rule)
        },
    }
    # LF on every platform, so the file compares equal to the committed one.
    sys.stdout.reconfigure(newline="\n")
    json.dump(out, sys.stdout, indent=2, sort_keys=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
