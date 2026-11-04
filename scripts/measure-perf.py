#!/usr/bin/env python3
"""Measures one realism-gate binary: its size and its run time over the
pinned in-sample corpus. Standard library only.

The corpus must already be fetched (scripts/bench-corpus.sh or eval/fetch.sh).
Run time is the wall-clock time of `check src/test --format json`, measured
RUNS times after one warm-up run; the median is the published figure. It
depends on the machine, so the host is recorded next to it, and only figures
from the same host are compared.

Usage:
  measure-perf.py BINARY LABEL OUT.json [--runs N]
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CORPUS = ROOT / ".corpus" / "defihacklabs"
PINNED = "2c99b565ae24ea2006adf181da20c4419b3edc30"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("binary")
    ap.add_argument("label")
    ap.add_argument("out")
    ap.add_argument("--runs", type=int, default=7)
    args = ap.parse_args()

    binary = Path(args.binary).resolve()
    head = subprocess.run(
        ["git", "-C", str(CORPUS), "rev-parse", "HEAD"], capture_output=True, check=True
    ).stdout.decode().strip()
    if head != PINNED:
        print(f"corpus is at {head}, expected {PINNED}", file=sys.stderr)
        return 2
    version = subprocess.run(
        [str(binary), "--version"], capture_output=True, check=True
    ).stdout.decode().strip()
    cmd = [str(binary), "check", "src/test", "--no-declaration", "--format", "json", "--fail-on", "never"]

    def once() -> float:
        start = time.perf_counter()
        subprocess.run(cmd, cwd=CORPUS, stdout=subprocess.DEVNULL, check=True)
        return (time.perf_counter() - start) * 1000

    once()
    runs = [round(once(), 1) for _ in range(args.runs)]
    out = {
        "label": args.label,
        "version": version,
        "binary_bytes": binary.stat().st_size,
        "corpus_commit": PINNED,
        "runs_ms": runs,
        "median_ms": round(statistics.median(runs), 1),
        "host": {
            "system": platform.system(),
            "machine": platform.machine(),
            "cpus": os.cpu_count(),
        },
    }
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
