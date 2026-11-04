#!/usr/bin/env python3
"""Draws the stratified sample of flags to label: per rule, at most CAP flags,
or all of them when the rule raised fewer.

The draw is deterministic and does not depend on any library's RNG: flags are
ordered by sha256(seed + ":" + flag id) and the first CAP of each rule are
taken. The seed is committed in eval/seed.txt.

Usage:
  sample.py FLAGS.jsonl[,FLAGS2.jsonl] [EXCLUDE.jsonl ...] > SAMPLE.jsonl

Several comma-separated FLAGS files are pooled first (by id), which is how one
sample covers the flags of two versions on the same corpus.

Flags whose id appears in an EXCLUDE file are not drawn; that is how a second,
disjoint sample is drawn from the same corpus.
"""

from __future__ import annotations

import hashlib
import json
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CAP = 30


def seed() -> str:
    return (ROOT / "eval" / "seed.txt").read_text(encoding="utf-8").strip()


def load(path: str) -> list[dict]:
    with open(path, encoding="utf-8") as fh:
        return [json.loads(line) for line in fh if line.strip()]


def draw(flags: list[dict], exclude: set[str], s: str, cap: int = CAP) -> list[dict]:
    by_rule: dict[str, list[dict]] = defaultdict(list)
    for f in flags:
        if f["id"] not in exclude:
            by_rule[f["rule"]].append(f)
    out = []
    for rule in sorted(by_rule):
        ranked = sorted(
            by_rule[rule],
            key=lambda f: hashlib.sha256(f"{s}:{f['id']}".encode()).hexdigest(),
        )
        out.extend(ranked[:cap])
    return out


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    exclude: set[str] = set()
    for path in argv[2:]:
        exclude |= {f["id"] for f in load(path)}
    sys.stdout.reconfigure(newline="\n")
    pool: dict[str, dict] = {}
    for path in argv[1].split(","):
        for f in load(path):
            pool.setdefault(f["id"], f)
    for f in draw(list(pool.values()), exclude, seed()):
        sys.stdout.write(json.dumps(f, ensure_ascii=False) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
