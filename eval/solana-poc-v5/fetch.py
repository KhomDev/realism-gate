#!/usr/bin/env python3
"""Fetch and verify eval/solana-poc-v5 pins.

Default: download missing pins into the gitignored `.corpus/solana-poc-v5/`,
then verify. `--check` verifies only and makes no network call.

Hashes are sha256 of `git cat-file blob` bytes (or of the downloaded page or
PDF bytes), never of a working-tree file. Nothing downloaded is executed.
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
HERE = ROOT / "eval" / "solana-poc-v5"
INV = HERE / "inventory.json"
UA = "independent-security-research (corpus pin verification)"

_spec = importlib.util.spec_from_file_location("solana_v5_builder", HERE / "build_inventory.py")
B = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(B)


def git(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], capture_output=True)


def clone_at(url: str, dest: Path, commit: str) -> None:
    if dest.joinpath(".git").is_dir():
        have = git("-C", str(dest), "rev-parse", "HEAD")
        if have.returncode == 0 and have.stdout.decode().strip() == commit:
            return
        raise SystemExit(f"{dest} is at another commit; move it aside first")
    dest.mkdir(parents=True, exist_ok=True)
    git("-C", str(dest), "init", "-q")
    git("-C", str(dest), "config", "core.autocrlf", "false")
    git("-C", str(dest), "remote", "add", "origin", url)
    r = git("-C", str(dest), "fetch", "-q", "--depth", "1", "origin", commit)
    if r.returncode != 0:
        raise SystemExit(f"{url}@{commit}: {r.stderr.decode().strip()}")
    git("-C", str(dest), "checkout", "-q", "--detach", "FETCH_HEAD")


def get(url: str) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=120) as resp:
        return resp.read()


def fetch() -> None:
    if not B.METEORA_HTML.is_file():
        B.METEORA_HTML.parent.mkdir(parents=True, exist_ok=True)
        B.METEORA_HTML.write_bytes(get(B.METEORA_URL))
    clone_at(B.CASHIO_REPO, B.CASHIO_DIR, B.CASHIO_COMMIT)
    clone_at(B.PORT_REPO, B.PORT_DIR, B.PORT_COMMIT)
    if not B.PDF_FILE.is_file():
        raw = (f"https://raw.githubusercontent.com/port-finance/sundial/"
               f"{B.PDF_COMMIT}/{B.PDF_PATH}")
        B.PDF_FILE.parent.mkdir(parents=True, exist_ok=True)
        B.PDF_FILE.write_bytes(get(raw))


def verify(doc: dict) -> list[str]:
    errors = []
    rebuilt = {u["id"]: u for u in B.build()["units"]}
    for u in doc["units"]:
        r = rebuilt.get(u["id"])
        if r is None:
            errors.append(f"{u['id']}: not reproducible from the cache")
        elif r["files"] != u["files"]:
            errors.append(f"{u['id']}: pin differs from the cache")
    # Sponsor status lines must still be in a fresh pdftotext of the pinned PDF.
    if shutil_which("pdftotext"):
        import tempfile
        with tempfile.TemporaryDirectory() as td:
            txt = Path(td) / "r.txt"
            subprocess.run(["pdftotext", "-layout", "-enc", "UTF-8", str(B.PDF_FILE), str(txt)],
                           check=True)
            text = txt.read_text(encoding="utf-8", errors="replace")
            for _, _, quote, _ in B.PORT_UNITS:
                if quote not in text:
                    errors.append(f"sponsor PDF missing quote: {quote}")
    else:
        errors.append("pdftotext not found; sponsor PDF text not re-verified")
    return errors


def shutil_which(name: str) -> bool:
    from shutil import which
    return which(name) is not None


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args(argv[1:])
    doc = json.loads(INV.read_text(encoding="utf-8"))
    if not args.check:
        fetch()
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
