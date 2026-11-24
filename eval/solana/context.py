#!/usr/bin/env python3
"""Prints the sampled flags with a few lines of source around each one, for a
label reader. The split is not printed, so the reader labels the same way for
dev and held-out flags.

Usage:
  eval/solana/context.py [--split heldout|dev|all] [--corpus NAME] [--rule RG02]
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


def split_of(seed: str, flag: dict) -> str:
    h = hashlib.sha256(f"{seed}|{flag['corpus']}|{flag['id']}".encode("utf-8")).hexdigest()
    return "dev" if int(h[:8], 16) % 2 == 0 else "heldout"


def evidence(corpus: str, flag_id: str) -> str | None:
    path = WORK / f"{corpus}.findings.json"
    if not path.is_file():
        return None
    report = json.loads(path.read_text(encoding="utf-8"))
    for f in report.get("findings", []):
        if "suppressed" in f:
            continue
        key = "|".join([corpus, f["rule"], f["file"], str(f["line"]),
                        str(f["column"]), f["evidence"]])
        if hashlib.sha256(key.encode("utf-8")).hexdigest()[:16] == flag_id:
            return f["evidence"]
    return None


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--split", choices=["heldout", "dev", "all"], default="heldout")
    p.add_argument("--corpus")
    p.add_argument("--rule")
    p.add_argument("--context", type=int, default=6)
    p.add_argument("--agreement-sample", type=int, default=0,
                   help="take this many labelled held-out flags by hash, for a blinded second pass")
    p.add_argument("--agreement-seed", default="rg-solana-agreement-2026-11-24")
    args = p.parse_args(argv[1:])
    pins = json.loads((HERE / "corpus.json").read_text(encoding="utf-8"))
    rows = []
    for corpus in sorted(pins["corpora"]):
        for line in (HERE / "samples" / f"{corpus}.jsonl").read_text(encoding="utf-8").splitlines():
            if line.strip():
                rows.append(json.loads(line))
    rows = [r for r in rows
            if (args.corpus is None or r["corpus"] == args.corpus)
            and (args.rule is None or r["rule"] == args.rule)
            and (args.split == "all" or split_of(pins["seed"], r) == args.split)]
    if args.agreement_sample:
        labelled = set()
        for path in (HERE / "labels").glob("*.jsonl"):
            for line in path.read_text(encoding="utf-8").splitlines():
                if line.strip():
                    labelled.add(json.loads(line)["id"])
        rows = [r for r in rows if r["id"] in labelled]
        rows.sort(key=lambda r: hashlib.sha256(
            f"{args.agreement_seed}:{r['id']}".encode("utf-8")).hexdigest())
        rows = rows[: args.agreement_sample]
    rows.sort(key=lambda r: (r["rule"], r["corpus"], r["file"], r["line"], r["id"]))
    for i, r in enumerate(rows, 1):
        print(f"=== [{i}/{len(rows)}] id={r['id']} {r['corpus']} {r['rule']} "
              f"{r['language']} {r['file']}:{r['line']}:{r['column']}")
        print(f"tests: {r['tests']}")
        ev = evidence(r["corpus"], r["id"])
        if ev:
            print(f"evidence: {ev}")
        base = ROOT / pins["corpora"][r["corpus"]]["dir"]
        src = base / r["file"]
        if not src.is_file():
            print("SOURCE NOT FOUND")
            continue
        lines = src.read_text(encoding="utf-8", errors="replace").splitlines()
        lo = max(1, r["line"] - args.context)
        hi = min(len(lines), r["line"] + args.context)
        for n in range(lo, hi + 1):
            mark = ">>" if n == r["line"] else "  "
            print(f"{mark} {n:5d} {lines[n - 1]}")
        print()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
