#!/usr/bin/env python3
"""Prints one sampled flag with the source around it, for a label reader.

Usage:
  eval/solana/show.py <flag-id> [--context N]
  eval/solana/show.py --corpus anchor --rule RG02 [--limit 10]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solana"
WORK = ROOT / ".corpus" / "solana-work"


def rows() -> list[dict]:
    out = []
    for path in sorted(HERE.glob("flags/*.jsonl")):
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.strip():
                out.append(json.loads(line))
    return out


def evidence(name: str, flag_id: str) -> str | None:
    path = WORK / f"{name}.findings.json"
    if not path.is_file():
        return None
    report = json.loads(path.read_text(encoding="utf-8"))
    for f in report.get("findings", []):
        if "suppressed" in f:
            continue
        # The id depends on the corpus, rule, file, line, column and evidence.
        # Matching on the visible fields plus evidence text is enough here.
        maybe = {
            "rule": f["rule"], "file": f["file"], "line": f["line"],
            "column": f["column"], "evidence": f["evidence"],
        }
        key = "|".join([name, maybe["rule"], maybe["file"], str(maybe["line"]),
                        str(maybe["column"]), maybe["evidence"]])
        if hashlib.sha256(key.encode("utf-8")).hexdigest()[:16] == flag_id:
            return f["evidence"]
    return None


def show(row: dict, context: int) -> None:
    print(json.dumps({k: v for k, v in row.items() if k != "evidence_sha256"}, indent=2))
    ev = evidence(row["corpus"], row["id"])
    if ev:
        print(f"evidence: {ev}")
    pins = json.loads((HERE / "corpus.json").read_text(encoding="utf-8"))
    base = ROOT / pins["corpora"][row["corpus"]]["dir"]
    src = base / row["file"]
    if not src.is_file():
        print(f"source not found: {src}", file=sys.stderr)
        return
    lines = src.read_text(encoding="utf-8", errors="replace").splitlines()
    lo = max(1, row["line"] - context)
    hi = min(len(lines), row["line"] + context)
    print(f"--- {row['file']}:{lo}-{hi} ---")
    for n in range(lo, hi + 1):
        mark = ">>" if n == row["line"] else "  "
        print(f"{mark} {n:5d} {lines[n - 1]}")


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("flag_id", nargs="?")
    p.add_argument("--corpus")
    p.add_argument("--rule")
    p.add_argument("--limit", type=int, default=10)
    p.add_argument("--context", type=int, default=8)
    args = p.parse_args(argv[1:])
    all_rows = rows()
    if args.flag_id:
        found = [r for r in all_rows if r["id"] == args.flag_id]
        if not found:
            print(f"no flag with id {args.flag_id}", file=sys.stderr)
            return 2
        show(found[0], args.context)
        return 0
    picked = [
        r for r in all_rows
        if (args.corpus is None or r["corpus"] == args.corpus)
        and (args.rule is None or r["rule"] == args.rule)
    ]
    for row in picked[: args.limit]:
        show(row, args.context)
        print()
    print(f"{len(picked)} matching flags", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
