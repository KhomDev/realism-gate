#!/usr/bin/env python3
"""P2 in-sample measurement over the opened v2 inventory (durable copy).

This is the RG-A2 helper (formerly a scratch script outside the repository)
ported into the repo so the measurement can be reproduced. It runs a named
realism-gate binary over every counted unit of `eval/solana-poc-v2`, exactly as
`eval/solana-poc-v2/run.py` does, and prints every RG02/RG03 flag together with
a comparison against the committed 1.3.0 run.

In-sample by construction: the v2 corpus is opened DEV data. The only
defensible claims this script supports are "N previously labelled or reviewed
false positives are gone" and "no other rule moved" — never a precision rate.

The binary hash is the measurement's identity. Record it with the output:
`sha256sum <binary>`; a stale `target/debug` binary once reproduced the old
RG03 flags, so a measurement that does not name its binary is discarded.

Usage:
  eval/solana-poc-v2/measure_p2.py BINARY [UNIT[,UNIT...]]

Outputs (default `--out` is `<repo>/.corpus/p2-measure`, gitignored):
  flags.jsonl   every active flag, one JSON object per line
  rows.jsonl    one row per unit: readable, tests, files, flag count
  summary.json  counts, the binary path, and the v1.3.0 comparison

No network. Deterministic except for the binary path in summary.json.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))

import inventory  # noqa: E402
import run as runner  # noqa: E402


def binary_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_jsonl(path: Path) -> list[dict]:
    if not path.is_file():
        return []
    return [
        json.loads(line)
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("binary")
    parser.add_argument("units", nargs="?", default="",
                        help="comma-separated unit ids; empty means every counted unit")
    parser.add_argument("--out", default=str(ROOT / ".corpus" / "p2-measure"))
    args = parser.parse_args(argv[1:])

    binary = Path(args.binary).resolve()
    if not binary.is_file():
        raise SystemExit(f"binary not found: {binary}")
    only = {u for u in args.units.split(",") if u} or None
    out = Path(args.out).resolve()

    inv = json.loads((HERE / "inventory.json").read_text(encoding="utf-8"))
    corpus = inventory.pins()
    # The work tree holds extracted Arena/Jito blocks; inventory.py defines it.
    runner.WORK = inventory.WORK
    if not inventory.WORK.is_dir():
        raise SystemExit(
            f"work tree missing: {inventory.WORK} (run eval/solana-poc-v2/fetch.sh and extract.py)"
        )

    rows: list[dict] = []
    flags: list[dict] = []
    for unit in inv["units"]:
        if "duplicate_of" in unit:
            continue
        if only and unit["id"] not in only:
            continue
        row, unit_flags, _report = runner.run_unit(str(binary), unit, corpus)
        rows.append(row)
        flags.extend(unit_flags)

    out.mkdir(parents=True, exist_ok=True)
    with open(out / "flags.jsonl", "w", encoding="utf-8", newline="\n") as handle:
        for flag in flags:
            handle.write(json.dumps(flag, ensure_ascii=False, sort_keys=True) + "\n")
    with open(out / "rows.jsonl", "w", encoding="utf-8", newline="\n") as handle:
        for row in rows:
            handle.write(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n")

    by_rule: dict[str, int] = {}
    for flag in flags:
        by_rule[flag["rule"]] = by_rule.get(flag["rule"], 0) + 1
    readable = sum(1 for row in rows if row["readable"])

    old_flags = read_jsonl(HERE / "flags" / "v1.3.0.jsonl")
    old_rows = read_jsonl(HERE / "runs" / "v1.3.0.jsonl")
    have = {(f["unit"], f["rule"], f["file"], f["line"]) for f in flags}
    old_rg02_rg03 = [f for f in old_flags if f["rule"] in ("RG02", "RG03")]
    still = [
        f for f in old_rg02_rg03
        if (f["unit"], f["rule"], f["file"], f["line"]) in have
    ]
    gone = [
        f for f in old_rg02_rg03
        if (f["unit"], f["rule"], f["file"], f["line"]) not in have
    ]

    summary = {
        "binary": str(binary),
        "binary_sha256": binary_sha256(binary),
        "corpus": "eval/solana-poc-v2 (opened DEV)",
        "units": len(rows),
        "readable": readable,
        "flags": len(flags),
        "flags_by_rule": dict(sorted(by_rule.items())),
        "old_readable": sum(1 for row in old_rows if row["readable"]),
        "old_rg02": sum(1 for f in old_flags if f["rule"] == "RG02"),
        "old_rg03": sum(1 for f in old_flags if f["rule"] == "RG03"),
        "still_rg02": sum(1 for f in still if f["rule"] == "RG02"),
        "still_rg03": sum(1 for f in still if f["rule"] == "RG03"),
        "gone_rg02": sum(1 for f in gone if f["rule"] == "RG02"),
        "gone_rg03": sum(1 for f in gone if f["rule"] == "RG03"),
    }
    (out / "summary.json").write_text(
        json.dumps(summary, indent=1, sort_keys=True) + "\n", encoding="utf-8", newline="\n"
    )

    for flag in flags:
        if flag["rule"] in ("RG02", "RG03"):
            print(f'{flag["unit"]}\t{flag["rule"]}\t{flag["file"]}:{flag["line"]}\t{flag["message"][:180]}')
    print(
        f'UNITS {summary["units"]} READABLE {readable} FLAGS {summary["flags"]} '
        f'BY_RULE {json.dumps(summary["flags_by_rule"], sort_keys=True)}'
    )
    print(
        f'OLD_READABLE {summary["old_readable"]} '
        f'OLD_RG02 {summary["old_rg02"]} OLD_RG03 {summary["old_rg03"]} '
        f'STILL_RG02 {summary["still_rg02"]} STILL_RG03 {summary["still_rg03"]} '
        f'GONE_RG02 {summary["gone_rg02"]} GONE_RG03 {summary["gone_rg03"]}'
    )
    print(f'OUT {out}')
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
