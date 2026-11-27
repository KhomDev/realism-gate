#!/usr/bin/env python3
"""Fetches and verifies every pin of eval/solana-poc-v4.

Default: download anything missing into the gitignored cache
`.corpus/solana-poc-v4/`, then verify. `--check`: verify only; no network.

Fetches: the Lavarage findings-repo issues through `gh api` (one call, the
operator's own GitHub auth, within the 10/min budget); the Cantina report PDF
by HTTPS. The PDF text is re-extracted with `pdftotext -layout -enc UTF-8`
(xpdf 4.06) into a temporary file and must equal the cached extraction.

Verification recomputes every unit's pin from the cache with the builder's own
functions and compares it with `inventory.json`. Nothing downloaded is ever
executed.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solana-poc-v4"
CACHE = ROOT / ".corpus" / "solana-poc-v4"
INV = HERE / "inventory.json"
UA = "independent-security-research (corpus pin verification)"

_spec = importlib.util.spec_from_file_location("solana_v4_builder", HERE / "build_inventory.py")
B = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(B)


def fetch() -> None:
    lav = CACHE / "c4" / "lavarage-issues.json"
    if not lav.is_file():
        out = subprocess.run(["gh", "api", f"repos/{B.LAV_REPO}/issues?state=all&per_page=100"],
                             capture_output=True, text=True, encoding="utf-8", errors="replace")
        if out.returncode != 0:
            raise SystemExit("gh api failed: " + out.stderr.strip())
        items = [{"number": i["number"], "title": i["title"],
                  "labels": [{"name": l["name"]} for l in i["labels"]], "body": i.get("body") or ""}
                 for i in json.loads(out.stdout)]
        items.sort(key=lambda i: i["number"])
        lav.parent.mkdir(parents=True, exist_ok=True)
        lav.write_text(json.dumps(items, indent=1, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    if not B.CANTINA_SF_PDF.is_file():
        req = urllib.request.Request(B.CANTINA_SF_URL, headers={"User-Agent": UA})
        with urllib.request.urlopen(req, timeout=120) as r:
            data = r.read()
        B.CANTINA_SF_PDF.parent.mkdir(parents=True, exist_ok=True)
        B.CANTINA_SF_PDF.write_bytes(data)
    if not B.CANTINA_SF_TXT.is_file():
        subprocess.run(["pdftotext", "-layout", "-enc", "UTF-8", str(B.CANTINA_SF_PDF), str(B.CANTINA_SF_TXT)], check=True)


def verify(doc: dict) -> list[str]:
    errors: list[str] = []
    rebuilt = {u["id"]: u for u in B.lavarage_units() + B.cantina_units()}
    for u in doc["units"]:
        r = rebuilt.get(u["id"])
        if r is None:
            errors.append(f"{u['id']}: not reproducible from the cache")
        elif r["files"] != u["files"]:
            errors.append(f"{u['id']}: pin differs from the cache")
    if shutil.which("pdftotext"):
        with tempfile.TemporaryDirectory() as td:
            t = Path(td) / "r.txt"
            subprocess.run(["pdftotext", "-layout", "-enc", "UTF-8", str(B.CANTINA_SF_PDF), str(t)], check=True)
            if t.read_bytes() != B.CANTINA_SF_TXT.read_bytes():
                errors.append("cantina PDF re-extraction differs from the cached text")
    else:
        errors.append("pdftotext not found; PDF extraction not re-verified")
    return errors


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
