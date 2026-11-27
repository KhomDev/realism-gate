#!/usr/bin/env python3
"""Fetch and verify eval/solidity-poc-v3 pins.

Default: clone the two pinned commits into the gitignored
`.corpus/solidity-poc-v3/`, then verify. `--check` verifies only and makes
no network call.

DeFiHackLabs is fetched at both the opened pin (d4bd931, to know which tests
are new) and the collection pin. Hashes are sha256 of `git cat-file blob`
bytes. Checkouts set core.autocrlf=false. Nothing downloaded is executed.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solidity-poc-v3"
INV = HERE / "inventory.json"

_spec = importlib.util.spec_from_file_location("solidity_v3_builder", HERE / "build_inventory.py")
B = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(B)


def git(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], capture_output=True)


def ensure(url: str, dest: Path, commits: list[str]) -> None:
    if not dest.joinpath(".git").is_dir():
        dest.mkdir(parents=True, exist_ok=True)
        git("-C", str(dest), "init", "-q")
        git("-C", str(dest), "config", "core.autocrlf", "false")
        git("-C", str(dest), "remote", "add", "origin", url)
    for commit in commits:
        have = git("-C", str(dest), "cat-file", "-e", f"{commit}^{{commit}}")
        if have.returncode == 0:
            continue
        r = git("-C", str(dest), "fetch", "-q", "--depth", "1", "origin", commit)
        if r.returncode != 0:
            raise SystemExit(f"{url}@{commit}: {r.stderr.decode().strip()}")
    git("-C", str(dest), "checkout", "-q", "--detach", commits[0])


def fetch() -> None:
    ensure(B.DFH_REPO, B.DFH_DIR, [B.DFH_COMMIT, B.DFH_OLD])
    ensure(B.TWABI_REPO, B.TWABI_DIR, [B.TWABI_COMMIT])


def verify(doc: dict) -> list[str]:
    errors = []
    rebuilt = {u["id"]: u for u in B.build()["units"]}
    for u in doc["units"]:
        r = rebuilt.get(u["id"])
        if r is None:
            errors.append(f"{u['id']}: not reproducible from the cache")
        elif r["files"] != u["files"]:
            errors.append(f"{u['id']}: pin differs from the cache")
    return errors


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args(argv[1:])
    if not args.check:
        fetch()
    doc = json.loads(INV.read_text(encoding="utf-8"))
    errors = verify(doc)
    if errors:
        print(f"{len(errors)} pin error(s):", file=sys.stderr)
        for e in errors[:40]:
            print("  " + e, file=sys.stderr)
        return 1
    print(f"all {len(doc['units'])} unit pins verified" + (" (offline)" if args.check else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
