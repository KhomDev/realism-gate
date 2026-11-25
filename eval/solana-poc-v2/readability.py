#!/usr/bin/env python3
"""RQ1 readability with a named binary, without touching a frozen run file.

Reads the pinned corpus cache (offline) through the corpus's own `run.py`
helpers and prints readable/unreadable units per tier. Use it to check what a
newer binary reads; it never writes runs/, flags/ or sample/ — a frozen
measurement stays frozen, and a new one is recorded in RESULTS.md with the
binary's version and hash.

Usage:
  eval/solana-poc-v2/readability.py <realism-gate-binary> [--corpus-dir DIR] [--json]
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("binary")
    ap.add_argument("--corpus-dir", default=str(HERE))
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    corpus_dir = Path(args.corpus_dir).resolve()
    sys.path.insert(0, str(corpus_dir))
    import inventory  # noqa: PLC0415 - resolved from the corpus directory
    import run as runner  # noqa: PLC0415

    binary = str(Path(args.binary).resolve())
    corpus = inventory.pins()
    inv = json.loads((corpus_dir / "inventory.json").read_text(encoding="utf-8"))
    rows = []
    for unit in inv["units"]:
        if "duplicate_of" in unit:
            continue
        row, _flags, _report = runner.run_unit(binary, unit, corpus)
        rows.append(
            {
                "unit": unit["id"],
                "tier": unit["tier"],
                "readable": row["readable"],
                "tests_checked": row["tests_checked"],
            }
        )
    tiers: dict[str, list[int]] = {}
    for row in rows:
        got = tiers.setdefault(row["tier"], [0, 0])
        got[1] += 1
        got[0] += 1 if row["readable"] else 0
    unreadable = [r["unit"] for r in rows if not r["readable"]]
    if args.json:
        print(json.dumps({"tiers": tiers, "unreadable": unreadable}, indent=1))
        return 0
    for tier in sorted(tiers):
        got, total = tiers[tier]
        print(f"{tier}: {got}/{total} readable")
    if unreadable:
        print("unreadable: " + ", ".join(unreadable))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
