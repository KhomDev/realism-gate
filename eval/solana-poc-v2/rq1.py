#!/usr/bin/env python3
"""RQ1 coverage for the sealed v2 corpus: readable units per tier and kind,
with Wilson 95% intervals, and the unreadable units by reason.

Reads runs/v<version>.jsonl and inventory.json only. No labels needed; this is
the coverage question, not precision.

Usage:
  eval/solana-poc-v2/rq1.py [--version 1.2.0]
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solana-poc-v2"
Z = 1.959964

REASON = {
    "binary-main": "Rust binary with fn main (no recognised PoC framework)",
    "ctf-solve-program": "on-chain attacker program, not a test",
    "rust-unit-test": "Rust unit test that calls no program function",
    "script": "TS/JS script that drives no recognised client",
    "test-fragment": "fragment that did not parse as a test body",
}


def wilson(k: int, n: int) -> list[float] | None:
    if n == 0:
        return None
    p = k / n
    denom = 1 + Z * Z / n
    centre = (p + Z * Z / (2 * n)) / denom
    half = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / denom
    return [round(max(0.0, centre - half), 4), round(min(1.0, centre + half), 4)]


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--version", default="1.2.0")
    args = p.parse_args(argv[1:])
    inv = json.loads((HERE / "inventory.json").read_text(encoding="utf-8"))
    units = {u["id"]: u for u in inv["units"] if "duplicate_of" not in u}
    runs = [json.loads(l) for l in
            (HERE / "runs" / f"v{args.version}.jsonl").read_text(encoding="utf-8").splitlines()
            if l.strip()]
    by_tier: dict[str, list[bool]] = defaultdict(list)
    by_kind: dict[str, list[bool]] = defaultdict(list)
    unreadable: list[tuple[str, str, str]] = []
    for r in runs:
        unit = units[r["unit"]]
        by_tier[unit["tier"]].append(r["readable"])
        by_kind[unit["kind"]].append(r["readable"])
        if not r["readable"]:
            reason = REASON.get(unit["kind"], unit["kind"])
            if r["parse_errors"]:
                reason = "does not parse"
            unreadable.append((unit["id"], unit["kind"], reason))
    out = {"version": args.version, "tiers": {}, "kinds": {}, "unreadable": unreadable}
    for tier, flags in sorted(by_tier.items()):
        out["tiers"][tier] = {"readable": sum(flags), "n": len(flags),
                              "wilson95": wilson(sum(flags), len(flags))}
    for kind, flags in sorted(by_kind.items()):
        out["kinds"][kind] = {"readable": sum(flags), "n": len(flags),
                              "wilson95": wilson(sum(flags), len(flags))}
    print(json.dumps(out, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
