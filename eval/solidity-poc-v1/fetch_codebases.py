#!/usr/bin/env python3
"""Materialise the Solidity v1 audited codebases at their pins.

`inventory.json` names a repo and commit per `codebase_id`; the pin cache
holds the findings repos and report pages, but the runner also needs the
sponsor snapshots checked out for `--model-target`. This script clones each
missing codebase at its pinned commit into the directory the runner expects
(`codebase_dir` in `run.py`).

It is network-only; downloaded code is never executed. A repo that cannot be
fetched is recorded and left absent; the runner then records
`checkout-absent` for its units rather than guessing.

Usage:
  python -I eval/solidity-poc-v1/fetch_codebases.py [--cache DIR] [--only ID]
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))
import run as runner  # noqa: E402


def git(args: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-c", "core.longpaths=true", *args],
        capture_output=True, text=True, encoding="utf-8", errors="replace",
    )


def clone(repo: str, commit: str, dest: Path) -> str:
    if dest.is_dir():
        head = git(["-C", str(dest), "rev-parse", "HEAD"]).stdout.strip()
        return "present" if head == commit else f"present-drift:{head[:12]}"
    dest.parent.mkdir(parents=True, exist_ok=True)
    proc = git(["clone", "--filter=blob:none", "--no-checkout", repo, str(dest)])
    if proc.returncode != 0:
        return f"clone-failed:{(proc.stderr or proc.stdout).strip().splitlines()[-1][:140]}"
    proc = git(["-C", str(dest), "fetch", "--depth", "1", "origin", commit])
    if proc.returncode != 0:
        return f"fetch-failed:{(proc.stderr or proc.stdout).strip().splitlines()[-1][:140]}"
    proc = git(["-C", str(dest), "checkout", "-f", "FETCH_HEAD"])
    if proc.returncode != 0:
        return f"checkout-failed:{(proc.stderr or proc.stdout).strip().splitlines()[-1][:140]}"
    head = git(["-C", str(dest), "rev-parse", "HEAD"]).stdout.strip()
    return f"head-mismatch:{head[:12]}" if head != commit else "cloned"


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cache", default=str(ROOT / ".corpus" / "solidity-poc-v1"))
    ap.add_argument("--only", action="append", default=[])
    args = ap.parse_args(argv[1:])
    inv = json.loads((HERE / "inventory.json").read_text(encoding="utf-8"))
    cache = Path(args.cache).resolve()
    counts: dict[str, int] = {}
    failures: list[str] = []
    for cid, entry in sorted(inv["codebases"].items()):
        if args.only and cid not in args.only:
            continue
        repo, commit = entry.get("repo"), entry.get("commit")
        if not repo or not commit:
            counts["no-pin"] = counts.get("no-pin", 0) + 1
            failures.append(f"{cid}: no repo/commit pin")
            continue
        dest = cache / "repos" / runner.codebase_dir(repo, cache)
        outcome = clone(repo, commit, dest)
        key = outcome.split(":")[0]
        counts[key] = counts.get(key, 0) + 1
        if key not in ("cloned", "present"):
            failures.append(f"{cid}: {outcome}")
        print(f"{cid}\t{outcome}\t{dest.name}", flush=True)
    print(json.dumps({"counts": counts, "failures": failures}, indent=1))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
