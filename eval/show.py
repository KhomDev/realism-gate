#!/usr/bin/env python3
"""Prints each flag of a sample with the source around it, for labelling or
for checking someone else's labels.

Usage:
  show.py CORPUS SAMPLE.jsonl [--before N] [--after N] [--id ID]

For RG08 (no assertion) the whole test function is printed, since the label
depends on everything the test does, not on one line.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "eval"))
import corpus  # noqa: E402


def function_span(lines: list[str], start: int) -> tuple[int, int]:
    """From the line declaring a function, the span up to its closing brace."""
    depth = 0
    opened = False
    for i in range(start, len(lines)):
        for ch in lines[i]:
            if ch == "{":
                depth += 1
                opened = True
            elif ch == "}":
                depth -= 1
        if opened and depth <= 0:
            return start, i
    return start, min(len(lines) - 1, start + 80)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("corpus", choices=sorted(corpus.pins()))
    ap.add_argument("sample")
    ap.add_argument("--before", type=int, default=8)
    ap.add_argument("--after", type=int, default=6)
    ap.add_argument("--id")
    args = ap.parse_args()
    base = corpus.directory(args.corpus)
    with open(args.sample, encoding="utf-8") as fh:
        flags = [json.loads(line) for line in fh if line.strip()]
    for f in flags:
        if args.id and f["id"] != args.id:
            continue
        lines = (base / f["file"]).read_text(encoding="utf-8", errors="replace").splitlines()
        n = f["line"] - 1
        if f["rule"] == "RG08":
            lo, hi = function_span(lines, n)
        else:
            lo, hi = max(0, n - args.before), min(len(lines) - 1, n + args.after)
        print(f"=== {f['id']}  {f['rule']} {f['severity']}  {f['file']}:{f['line']}  {f['suite']}::{','.join(f['tests'])}")
        print(f"    evidence: {f.get('evidence', '(not stored)')}")
        for i in range(lo, hi + 1):
            mark = ">>" if i == n else "  "
            print(f"{mark}{i + 1:5d} {re.sub(r'\s+$', '', lines[i])}")
        print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
