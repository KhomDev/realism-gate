#!/usr/bin/env python3
"""Fetches and verifies every pin of eval/solidity-poc-v2.

Default: download anything missing into the gitignored cache
`.corpus/solidity-poc-v2/` (git clones at the pinned commits, raw files at the
pinned commits, write-up pages), then verify. `--check`: verify only; makes no
network call.

Verification recomputes every unit's pin from the cache with the builder's own
functions and compares it with `inventory.json`, checks every git checkout is
at its pinned commit, and checks every raw-file hash. Nothing downloaded is
ever executed; checkouts are read by `git` and by hashing only.

Note: write-up pages (kind page-pre-block) are dynamic HTML; a later online
re-fetch can drift even when the PoC block is unchanged. The block hash is the
pin that matters; the page hash pins the cached copy.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import subprocess
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solidity-poc-v2"
CACHE = ROOT / ".corpus" / "solidity-poc-v2"
INV = HERE / "inventory.json"
UA = "independent-security-research (corpus pin verification)"

_spec = importlib.util.spec_from_file_location("solidity_v2_builder", HERE / "build_inventory.py")
B = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(B)


def git(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], capture_output=True, text=True, encoding="utf-8", errors="replace")


def clone_at(url: str, dest: Path, commit: str) -> None:
    if dest.is_dir():
        h = git("-C", str(dest), "rev-parse", "HEAD")
        if h.returncode == 0 and h.stdout.strip() == commit:
            return
        raise SystemExit(f"{dest} exists at another commit; move it aside first")
    dest.mkdir(parents=True)
    git("-C", str(dest), "init", "-q")
    git("-C", str(dest), "config", "core.longpaths", "true")
    git("-C", str(dest), "remote", "add", "origin", url)
    r = git("-C", str(dest), "fetch", "-q", "--depth", "1", "origin", commit)
    if r.returncode != 0:
        raise SystemExit(f"{url}@{commit}: {r.stderr.strip()}")
    git("-C", str(dest), "checkout", "-q", "FETCH_HEAD")


def get(url: str) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=120) as r:
        return r.read()


def fetch(doc: dict) -> None:
    heads = B.tsv(CACHE / "meta" / "sherlock-judging-heads.tsv") if (CACHE / "meta" / "sherlock-judging-heads.tsv").is_file() else {}
    for u in doc["units"]:
        f = u["files"][0]
        if f["kind"] == "sherlock-report-section-block":
            name = f["repo"].rsplit("/", 1)[1]
            clone_at(f["repo"] + ".git", CACHE / "sherlock" / name, f["commit"])
        elif f["kind"] == "repo-file" and "learn-evm-attacks" in f["repo"]:
            clone_at(f["repo"] + ".git", CACHE / "x" / "learn-evm-attacks", f["commit"])
    for w in B.WRITEUPS:
        p = CACHE / w["pin"]["cache"]
        if p.is_file():
            continue
        p.parent.mkdir(parents=True, exist_ok=True)
        pin = w["pin"]
        url = pin["url"] if pin["kind"] == "page-pre-block" else \
            f"{pin['repo'].replace('github.com', 'raw.githubusercontent.com')}/{pin['commit']}/{pin['path']}"
        p.write_bytes(get(url))
    del heads
    # Dynamic pages: re-fetch and require the pinned block hash to reproduce.
    for w in B.WRITEUPS:
        if w["pin"]["kind"] != "page-pre-block":
            continue
        fresh = get(w["pin"]["url"]).decode("utf-8", errors="replace")
        sel = B.select_sol_block(B.page_markdown(fresh))
        want = next(u for u in doc["units"] if u["id"] == w["id"])["files"][0]["block_sha256"]
        if sel is None or B.norm_hash(sel["code"]) != want:
            raise SystemExit(f"{w['id']}: the live page no longer reproduces block_sha256")


def verify(doc: dict) -> list[str]:
    errors: list[str] = []
    cb: dict = {}
    sher, _ = B.sherlock_units(cb)
    rebuilt = {u["id"]: u for u in sher + B.extra_units(cb)}
    for u in doc["units"]:
        r = rebuilt.get(u["id"])
        if r is None:
            errors.append(f"{u['id']}: not reproducible from the cache")
            continue
        if r["files"] != u["files"]:
            errors.append(f"{u['id']}: pin differs from the cache")
        f = u["files"][0]
        if f["kind"] == "sherlock-report-section-block":
            d = CACHE / "sherlock" / f["repo"].rsplit("/", 1)[1]
            h = git("-C", str(d), "rev-parse", "HEAD")
            if h.returncode != 0 or h.stdout.strip() != f["commit"]:
                errors.append(f"{u['id']}: judging checkout not at pinned commit")
            elif hashlib.sha256(B.git_blob(d, f["commit"], "README.md") or b"").hexdigest() != f["report_sha256"]:
                errors.append(f"{u['id']}: report hash drift")
        elif f["kind"] == "repo-file":
            d = CACHE / "x" / "learn-evm-attacks"
            h = git("-C", str(d), "rev-parse", "HEAD")
            if h.returncode != 0 or h.stdout.strip() != f["commit"]:
                errors.append(f"{u['id']}: checkout not at pinned commit")
    return errors


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args(argv[1:])
    doc = json.loads(INV.read_text(encoding="utf-8"))
    if not args.check:
        fetch(doc)
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
