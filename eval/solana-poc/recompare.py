#!/usr/bin/env python3
"""In-sample re-measurement on the P1 PoC corpus: 1.0.4 (frozen study) versus
1.2.0. Readability per tier, flags that disappeared and appeared, and what the
disappeared flags were labelled in the P1 study.

Offline; reads committed runs/flags/labels only.

Usage:
  eval/solana-poc/recompare.py [--json]
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solana-poc"
OLD, NEW = "1.0.4", "1.2.0"


def rows(name: str) -> list[dict]:
    path = HERE / "runs" / f"v{name}.jsonl"
    return [json.loads(l) for l in path.read_text(encoding="utf-8").splitlines() if l.strip()]


def flags(name: str) -> list[dict]:
    path = HERE / "flags" / f"v{name}.jsonl"
    return [json.loads(l) for l in path.read_text(encoding="utf-8").splitlines() if l.strip()]


def labels() -> dict[str, dict]:
    out: dict[str, dict] = {}
    for path in (HERE / "labels").glob("*.jsonl"):
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.strip():
                row = json.loads(line)
                out[row["id"]] = row
    return out


def readability(rs: list[dict]) -> dict:
    by: dict[str, list[bool]] = {}
    for r in rs:
        by.setdefault(r["tier"], []).append(r["readable"])
    return {t: {"readable": sum(v), "n": len(v)} for t, v in sorted(by.items())}


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--json", action="store_true")
    args = p.parse_args(argv[1:])
    old_runs, new_runs = rows(OLD), rows(NEW)
    old_flags, new_flags = flags(OLD), flags(NEW)
    lab = labels()
    old_ids = {f["id"] for f in old_flags}
    new_ids = {f["id"] for f in new_flags}
    gone = [f for f in old_flags if f["id"] not in new_ids]
    added = [f for f in new_flags if f["id"] not in old_ids]
    gone_labels = Counter(lab.get(f["id"], {}).get("label", "unlabelled") for f in gone)
    gone_rules = Counter(f["rule"] for f in gone)
    added_rules = Counter(f["rule"] for f in added)
    doc = {
        "schema": "realism-gate.solana-poc-recompare/v1",
        "populations": "in-sample: the P1 PoC corpus, held-out and dev together",
        "old": OLD,
        "new": NEW,
        "readability": {"old": readability(old_runs), "new": readability(new_runs)},
        "flags": {"old": len(old_flags), "new": len(new_flags),
                  "disappeared": len(gone), "added": len(added)},
        "disappeared_labels": dict(sorted(gone_labels.items())),
        "disappeared_by_rule": dict(sorted(gone_rules.items())),
        "added_by_rule": dict(sorted(added_rules.items())),
        "severity_new": dict(sorted(Counter(f["severity"] for f in new_flags).items())),
    }
    if args.json:
        print(json.dumps(doc, indent=2))
    else:
        print(json.dumps(doc, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
