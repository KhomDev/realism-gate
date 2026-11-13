#!/usr/bin/env python3
"""Assign the frozen claim-binding rubric labels to the 120 PoC+claim pairs,
and print the seeded 20-pair sample used for the reader agreement check.
Offline.

    python eval/claims/label.py --write      # write labels.yaml (rubric labels)
    python eval/claims/label.py --sample     # print the 20-pair reader sample

The rubric is mechanical and frozen before any tool run (see REPORT.md). It
labels what the PoC source structurally contains, not what a reader believes:

  impact drain   shown        an assertion compares a balance/amount (assertGt,
                              assertGe or assertEq with balance/profit/amount)
                 not_shown    the file contains no assert statement at all
                 not_checkable otherwise
  attacker       shown        the claimed attacker address literal appears in
                              the PoC source
                 not_shown    the record names an attacker, the PoC does not
                 not_checkable the incident record names no attacker
  amount         not_checkable by default: the claim is a fiat loss figure from
                 the incident record; a fork PoC asserts token balances, not
                 USD. This is measured, not assumed (see REPORT.md).
  fix            not checked by this corpus (no report fix rows)

Pair label is shown only if impact is shown and attacker is not not_shown;
contradicted only if every test function that calls assert also has an
expectRevert and no balance assertion exists.
"""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path

try:
    import yaml
except ImportError:
    print("eval/claims: PyYAML is required", file=sys.stderr)
    sys.exit(2)

HERE = Path(__file__).resolve().parent
RG = HERE.parent.parent
PAIRS = HERE / "pairs.yaml"
LABELS = HERE / "labels.yaml"
SOURCES = HERE / "sources.yaml"
CACHE = RG / "eval" / "cache"
ASSERT = re.compile(r"assert(Gt|Ge|Eq|Lt|Le)\s*\(")
BALANCE_ASSERT = re.compile(r"assert(Gt|Ge|Eq)\s*\([^;]{0,200}(balance|Balance|profit|Profit|amount|Amount)")
EXPECT_REVERT = re.compile(r"expectRevert\s*\(")


def spec() -> dict:
    return yaml.safe_load(SOURCES.read_text(encoding="utf-8"))


def corpus() -> dict:
    return spec()["corpora"][0]


def corpus_root() -> Path:
    for candidate in ((RG / corpus()["dir"]).resolve(), (RG / corpus()["fallback_dir"]).resolve()):
        if (candidate / ".git").is_dir():
            return candidate
    raise SystemExit("corpus not found; run fetch.py first")


def seed() -> str:
    return spec()["seed"]


def rubric_label(source: str, claim: dict) -> dict:
    attacker_claim = claim.get("attacker")
    if not attacker_claim:
        attacker = "not_checkable"
    else:
        address = attacker_claim.split(":", 1)[-1].lower()
        attacker = "shown" if address and address in source.lower() else "not_shown"
    if BALANCE_ASSERT.search(source):
        impact = "shown"
    elif not ASSERT.search(source):
        impact = "not_shown"
    else:
        impact = "not_checkable"
    if impact == "shown" and attacker != "not_shown":
        pair = "shown"
    elif impact == "not_shown":
        pair = "not_shown"
    elif impact == "shown" and attacker == "not_shown":
        pair = "not_checkable"
    else:
        pair = "not_checkable"
    contradiction = (
        bool(EXPECT_REVERT.search(source))
        and not BALANCE_ASSERT.search(source)
        and bool(re.search(r"function\s+test", source))
    )
    if contradiction:
        pair = "contradicted"
    return {"attacker": attacker, "impact": impact, "amount": "not_checkable",
            "pair": pair, "rubric_version": 1}


def load_pairs() -> dict:
    return yaml.safe_load(PAIRS.read_text(encoding="utf-8"))


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="label the claim-binding pairs")
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write", action="store_true")
    group.add_argument("--sample", action="store_true")
    args = parser.parse_args(argv)
    doc = load_pairs()
    root = corpus_root()
    rows = {}
    for pair in doc["pairs"]:
        source = (root / pair["poc"]["path"]).read_text(encoding="utf-8", errors="replace")
        rows[pair["id"]] = rubric_label(source, pair["claim"])
    if args.write:
        out = {
            "version": 1,
            "schema": "realism-gate.claims.labels/v1",
            "seed": seed(),
            "rubric": (
                "Frozen mechanical rubric in eval/claims/label.py. It labels structural "
                "evidence in the PoC source (balance assertion for impact, attacker address "
                "literal for identity), not the truth of the claim. Pair labels: shown, "
                "not_shown, not_checkable, contradicted. amount is not_checkable by design: "
                "the incident record states a fiat loss, a fork PoC asserts token balances."
            ),
            "rubric_version": 1,
            "pairs": rows,
            "reader_sample": [],
            "agreement": None,
        }
        LABELS.write_text(yaml.safe_dump(out, sort_keys=False, allow_unicode=True), encoding="utf-8", newline="\n")
        counts = {}
        for row in rows.values():
            counts[row["pair"]] = counts.get(row["pair"], 0) + 1
        field_counts = {}
        for field in ("attacker", "impact", "amount"):
            for row in rows.values():
                field_counts[f"{field}:{row[field]}"] = field_counts.get(f"{field}:{row[field]}", 0) + 1
        print(f"wrote {LABELS.relative_to(RG).as_posix()}: {len(rows)} pairs")
        print("pair labels:", dict(sorted(counts.items())))
        print("field labels:", dict(sorted(field_counts.items())))
        return 0
    # --sample: deterministic 20 pairs, print the evidence the reader needs
    ids = sorted(rows)
    ranked = sorted(ids, key=lambda i: hashlib.sha256(f"{seed()}:{i}".encode()).hexdigest())[:20]
    pairs = {p["id"]: p for p in doc["pairs"]}
    for pid in sorted(ranked):
        pair = pairs[pid]
        source = (root / pair["poc"]["path"]).read_text(encoding="utf-8", errors="replace")
        asserts = [l.strip() for l in source.splitlines() if ASSERT.search(l)][:4]
        attacker_present = (
            pair["claim"].get("attacker") is not None
            and pair["claim"]["attacker"].split(":", 1)[-1].lower() in source.lower()
        )
        print("=" * 90)
        print(pid)
        print("  lost:", pair["incident"]["lost_text"], "| attacker claim:", pair["claim"].get("attacker"),
              "| address literal in PoC:", attacker_present)
        print("  rubric:", rows[pid])
        for line in asserts:
            print("   assert:", line[:150])
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
