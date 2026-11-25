#!/usr/bin/env python3
"""Prints sampled PoC flags with the source around each one, for a label
reader. The split label is never printed; --split only selects which flags.

Usage:
  eval/solana-poc/context.py [--split heldout|dev|all] [--unit ID] [--rule RG02]
                             [--ids FILE] [--context N]
  eval/solana-poc/context.py --agreement-sample 30   blinded sample for the second pass
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import inventory  # noqa: E402
import run  # noqa: E402

HERE = inventory.HERE
WORK = inventory.WORK
AGREEMENT_SEED = "rg-solana-poc-agreement-2026-11-25"


def split_of(seed: str, flag: dict) -> str:
    h = hashlib.sha256(f"{seed}|{flag['source']}|{flag['id']}".encode("utf-8")).hexdigest()
    return "dev" if int(h[:8], 16) % 2 == 0 else "heldout"


def load_jsonl(path: Path) -> list[dict]:
    with open(path, encoding="utf-8") as fh:
        return [json.loads(line) for line in fh if line.strip()]


def evidence(reports: dict, flag: dict) -> str | None:
    for f in reports.get(flag["unit"], {}).get("findings", []):
        if "suppressed" in f:
            continue
        if run.flag_id(flag["unit"], f) == flag["id"]:
            return f["evidence"] + " || " + f.get("message", "")
    return None


def agreement_sample(n: int) -> list[str]:
    labelled = set()
    for path in (HERE / "labels").glob("*.jsonl"):
        labelled |= {r["id"] for r in load_jsonl(path)}
    ids = sorted(labelled, key=lambda i: hashlib.sha256(f"{AGREEMENT_SEED}:{i}".encode()).hexdigest())
    return ids[:n]


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--split", choices=["heldout", "dev", "all"], default="heldout")
    p.add_argument("--unit")
    p.add_argument("--rule")
    p.add_argument("--ids")
    p.add_argument("--context", type=int, default=8)
    p.add_argument("--agreement-sample", type=int, default=0)
    args = p.parse_args(argv[1:])
    corpus = inventory.pins()
    inv = {u["id"]: u for u in json.loads((HERE / "inventory.json").read_text(encoding="utf-8"))["units"]}
    rows = load_jsonl(HERE / "sample.jsonl")
    if args.agreement_sample:
        keep = set(agreement_sample(args.agreement_sample))
        rows = [r for r in rows if r["id"] in keep]
    else:
        rows = [r for r in rows
                if (args.unit is None or r["unit"] == args.unit)
                and (args.rule is None or r["rule"] == args.rule)
                and (args.split == "all" or split_of(corpus["seed"], r) == args.split)]
    if args.ids:
        keep = set(Path(args.ids).read_text(encoding="utf-8").split())
        rows = [r for r in rows if r["id"] in keep]
    reports = json.loads((WORK / "findings.json").read_text(encoding="utf-8"))
    rows.sort(key=lambda r: (r["unit"], r["file"], r["line"], r["rule"], r["id"]))
    for i, r in enumerate(rows, 1):
        u = inv[r["unit"]]
        print(f"=== [{i}/{len(rows)}] id={r['id']} {r['unit']} tier={r['tier']} {r['rule']} "
              f"{r['severity']} {r['language']} {r['file']}:{r['line']}:{r['column']}")
        print(f"tests: {r['tests']}")
        ev = evidence(reports, r)
        if ev:
            print(f"evidence: {ev}")
        base = run.base_dir(u, corpus)
        src = base / r["file"]
        if not src.is_file():
            print("SOURCE NOT FOUND")
            continue
        lines = src.read_text(encoding="utf-8", errors="replace").splitlines()
        lo = max(1, r["line"] - args.context)
        hi = min(len(lines), r["line"] + args.context)
        for n in range(lo, hi + 1):
            mark = ">>" if n == r["line"] else "  "
            print(f"{mark} {n:5d} {lines[n - 1][:160]}")
        print()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
