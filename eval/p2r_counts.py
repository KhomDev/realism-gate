#!/usr/bin/env python3
"""Flag counts and readability for frozen runs. **No precision, ever.**

Reads the files `run.py` wrote (`meta.json`, `runs.jsonl` or `runs/<v>.jsonl`,
`flags.jsonl` or `flags/<v>.jsonl`) and reports, per corpus: units by tier,
readability (units with at least one test read / units, Wilson 95%), flags by
rule and tier, and the number of tier-J/X flags that go to the labellers. A flag
is a count until a labeller says it is a true positive; this script never
labels, never reads a label, and prints no precision. n < 5 is printed as a
count, as in the analysis plan.

Usage:
  python -I eval/p2r_counts.py --run NAME=DIR [--run NAME=DIR ...] [--json out.json]
  python -I eval/p2r_counts.py --self-test
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from collections import Counter, defaultdict
from pathlib import Path

Z = 1.959964


def wilson(k: int, n: int):
    if n == 0:
        return None
    p = k / n
    d = 1 + Z * Z / n
    c = (p + Z * Z / (2 * n)) / d
    h = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / d
    return [round(max(0.0, c - h), 4), round(min(1.0, c + h), 4)]


def jsonl(path: Path) -> list[dict]:
    return [json.loads(x) for x in path.read_text(encoding="utf-8").splitlines() if x.strip()]


def one(run: Path, flat: str, folder: str) -> Path:
    cands = [p for p in [run / flat] if p.is_file()]
    if (run / folder).is_dir():
        cands += sorted((run / folder).glob("*.jsonl"))
    if len(cands) != 1:
        raise SystemExit(f"{run}: expected exactly one of {flat} / {folder}/*.jsonl, found {[str(c) for c in cands]}")
    return cands[0]


def summarise(run: Path) -> dict:
    meta = json.loads((run / "meta.json").read_text(encoding="utf-8"))
    flags = jsonl(one(run, "flags.jsonl", "flags"))
    units = jsonl(one(run, "runs.jsonl", "runs"))
    by_tier_units = Counter(u["tier"] for u in units)
    readable = [u for u in units if u.get("readable", u.get("tests_read", 0))]
    by_rule = defaultdict(Counter)
    for f in flags:
        by_rule[f["rule"]][f["tier"]] += 1
    jx = sum(1 for f in flags if f["tier"] in ("J", "X"))
    return {
        "splits": meta.get("splits"),
        "binary_sha256": meta.get("binary_sha256"),
        "round": meta.get("round"),
        "units": len(units),
        "units_by_tier": dict(sorted(by_tier_units.items())),
        "readable": len(readable),
        "readable_wilson95": wilson(len(readable), len(units)),
        "flags": len(flags),
        "flags_by_tier": dict(sorted(Counter(f["tier"] for f in flags).items())),
        "flags_by_rule": {r: dict(sorted(c.items())) for r, c in sorted(by_rule.items())},
        "flags_to_label_JX": jx,
        "severities": dict(sorted(Counter(f["severity"] for f in flags).items())),
        "precision": "not computed: flags are unlabelled",
    }


def self_test() -> int:
    assert wilson(0, 0) is None
    lo, hi = wilson(5, 10)
    assert 0.23 < lo < 0.24 and 0.76 < hi < 0.77, (lo, hi)
    print("self-test ok: wilson")
    return 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", action="append", default=[], help="NAME=DIR")
    ap.add_argument("--json")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv[1:])
    if args.self_test:
        return self_test()
    if not args.run:
        ap.error("--run NAME=DIR is required")
    out = {}
    for spec in args.run:
        name, _, d = spec.partition("=")
        out[name] = summarise(Path(d))
    for name, s in out.items():
        print(f"{name}: splits={s['splits']} units={s['units']} {s['units_by_tier']} readable={s['readable']}/{s['units']} "
              f"{s['readable_wilson95']} flags={s['flags']} {s['flags_by_tier']} to-label(J/X)={s['flags_to_label_JX']}")
        for rule, c in s["flags_by_rule"].items():
            print(f"    {rule}: {c}")
    if args.json:
        Path(args.json).write_text(json.dumps(out, indent=1, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))