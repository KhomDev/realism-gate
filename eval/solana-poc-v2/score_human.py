#!/usr/bin/env python3
"""Score a human labelling of the v2 held-out flags against the primary labels.

The human reads `labels/HUMAN_PACKET.md`, writes `labels/human_heldout.csv`
with `id,label,reason`, and only then runs this script. It reports raw
agreement and Cohen's kappa over `true-positive` / `false-positive` /
`unclear`, per rule, and names every id the human did not label.

`--self-test` checks the arithmetic on a fixed fixture and reads no corpus.
"""

from __future__ import annotations

import argparse
import csv
import json
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
CATEGORIES = ("true-positive", "false-positive", "unclear")


def kappa(pairs: list[tuple[str, str]]) -> tuple[float, float]:
    """(raw agreement, Cohen's kappa) over the labelled pairs."""
    n = len(pairs)
    if n == 0:
        return 0.0, 0.0
    po = sum(1 for a, b in pairs if a == b) / n
    ch = Counter(a for a, _ in pairs)
    cp = Counter(b for _, b in pairs)
    pe = sum((ch[c] / n) * (cp[c] / n) for c in CATEGORIES)
    if pe >= 1.0:
        return po, 1.0 if po == 1.0 else 0.0
    return po, (po - pe) / (1.0 - pe)


def self_test() -> int:
    pairs = [
        ("true-positive", "true-positive"),
        ("true-positive", "false-positive"),
        ("false-positive", "false-positive"),
    ]
    po, k = kappa(pairs)
    ok = abs(po - 2 / 3) < 1e-9 and abs(k - 0.4) < 1e-9
    print(f"self-test agreement={po:.4f} kappa={k:.4f} {'ok' if ok else 'FAIL'}")
    return 0 if ok else 1


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--human", default=str(HERE / "labels/human_heldout.csv"))
    ap.add_argument("--primary", default=str(HERE / "labels/heldout.jsonl"))
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()

    rows = [json.loads(line) for line in open(args.primary, encoding="utf-8") if line.strip()]
    primary = {r["id"]: r["label"] for r in rows}
    rules = {r["id"]: r["rule"] for r in rows}
    human: dict[str, str] = {}
    with open(args.human, encoding="utf-8", newline="") as fh:
        for row in csv.DictReader(fh):
            label = (row.get("label") or "").strip().lower()
            if label not in CATEGORIES:
                raise SystemExit(f"{row.get('id')}: label must be one of {CATEGORIES}")
            human[row["id"].strip()] = label

    missing = sorted(set(primary) - set(human))
    if missing:
        print(f"missing {len(missing)} of {len(primary)} ids: {', '.join(missing)}")
        return 2
    extra = sorted(set(human) - set(primary))
    if extra:
        raise SystemExit(f"ids not in the held-out set: {', '.join(extra)}")

    pairs = [(primary[i], human[i]) for i in sorted(primary)]
    po, k = kappa(pairs)
    print(f"n={len(pairs)} agreement={po:.4f} kappa={k:.4f}")
    for rid in sorted(set(rules.values())):
        ids = [i for i in primary if rules[i] == rid]
        agree = sum(1 for i in ids if primary[i] == human[i])
        print(f"{rid}: {agree}/{len(ids)} agreement")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
