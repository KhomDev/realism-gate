#!/usr/bin/env python3
"""Run a realism-gate binary over the solana-poc-v4 corpus (P2R-2 additions).

This is the v3 runner (`eval/solana-poc-v3/run.py`) with one change: unit
extraction understands v4's pin kinds and re-derives every block from the pin
cache with the corpus's own builder (`build_inventory.py`), refusing any block
whose normalised hash differs from the sealed inventory.

- `issue-block` (Code4rena Lavarage findings issues): the cached issue body is
  hashed against `body_sha256`, the content selector re-picks the block, and
  the block hash must match.
- `pdf-poc-span` (Cantina tier A): the cached `pdftotext -layout` text is
  re-cut into the finding's Proof-of-Concept span exactly as the builder does.

Duplicate submissions (a Code4rena `duplicate-N` issue whose finding has a
representative in the corpus) are skipped: a finding is one unit. Tier A units
run but are reported apart; they never enter a J or X cell (Amendment 2).

The split guard is v3's: `--split` defaults to DEV, and a sealed slice needs
`--round`, `--version` and `--binary-sha256`.

Usage:
  python -I eval/solana-poc-v4/run.py BINARY --version p2r2-dev --split DEV \
      --cache <worktree>/.corpus/solana-poc-v4 --out-dir <dir>
  python -I eval/solana-poc-v4/run.py --check --cache <cache>
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


V3RUN = _load("p2r_v3_run", ROOT / "eval" / "solana-poc-v3" / "run.py")
B = _load("p2r_v4_build", HERE / "build_inventory.py")

EXT = {"rust": "rs", "typescript": "ts", "javascript": "js"}


def _lavarage_code(unit: dict, pin: dict, cache: Path) -> str:
    issues = json.loads((cache / "c4" / "lavarage-issues.json").read_text(encoding="utf-8"))
    issue = next((i for i in issues if i["number"] == pin["issue"]), None)
    if issue is None:
        raise SystemExit(f"{unit['id']}: issue {pin['issue']} missing from the cache")
    body = issue.get("body") or ""
    if hashlib.sha256(body.encode("utf-8")).hexdigest() != pin["body_sha256"]:
        raise SystemExit(f"{unit['id']}: issue body drift")
    sel = B.select_block(body, issue.get("title") or "")
    if sel is None or sel["block"] != pin["block"]:
        raise SystemExit(f"{unit['id']}: the selector no longer picks block {pin['block']}")
    if B.norm_hash(sel["code"]) != pin["block_sha256"]:
        raise SystemExit(f"{unit['id']}: block hash drift")
    return sel["code"]


def _cantina_code(unit: dict, pin: dict, cache: Path) -> str:
    pdf = cache / B.CANTINA_SF_PDF.relative_to(B.CACHE)
    txt = cache / B.CANTINA_SF_TXT.relative_to(B.CACHE)
    if hashlib.sha256(pdf.read_bytes()).hexdigest() != pin["pdf_sha256"]:
        raise SystemExit(f"{unit['id']}: PDF drift")
    text = txt.read_text(encoding="utf-8", errors="replace").replace("\x0c", "\n")
    for num, title, sec in B.cantina_sections(text):
        if num != pin["section"]:
            continue
        i = sec.find("Proof of Concept:")
        j = sec.find("Recommendation:", i)
        span = sec[i:j if j > 0 else len(sec)]
        log = B.V3.LOG_LINE.search(span)
        if log:
            span = span[:log.start()]
        sel = B.select_block("```\n" + span + "\n```", title)
        if sel is None or B.norm_hash(sel["code"]) != pin["block_sha256"]:
            raise SystemExit(f"{unit['id']}: PoC span hash drift")
        return sel["code"]
    raise SystemExit(f"{unit['id']}: section {pin['section']} missing")


def extract(unit: dict, cache: Path, out: Path) -> tuple[list[str], Path, bool]:
    pin = unit["files"][0]
    if pin["kind"] == "issue-block":
        code = _lavarage_code(unit, pin, cache)
    elif pin["kind"] == "pdf-poc-span":
        code = _cantina_code(unit, pin, cache)
    else:
        raise SystemExit(f"{unit['id']}: unknown pin kind {pin['kind']}")
    name = f"{unit['id'].lower()}.{EXT.get(pin.get('lang'), 'txt')}"
    (out / name).write_text(code, encoding="utf-8", newline="\n")
    return [name], out, unit.get("kind") == "test-fragment"


def representatives(units: list[dict]) -> list[dict]:
    """Drop a duplicate submission whose finding's representative is present."""
    ids = {u["id"] for u in units}
    keep = []
    for u in units:
        key = u.get("group_key")
        num = u["files"][0].get("issue")
        if key is not None and num is not None and key != num:
            rep = f"J-c4-lavarage-{key}"
            if rep in ids:
                continue
        keep.append(u)
    return keep


def main(argv: list[str]) -> int:
    V3RUN.HERE = HERE
    V3RUN.CACHE = ROOT / ".corpus" / "solana-poc-v4"
    V3RUN.SEED = "rg-solana-poc-v4-2026-11-27"
    V3RUN.extract = extract
    inventory = V3RUN.load_json(HERE / "inventory.json")
    original = inventory["units"]
    kept = representatives(original)
    skipped = len(original) - len(kept)
    if skipped:
        print(f"skipping {skipped} duplicate submission(s); one unit per judged finding")
    real_load = V3RUN.load_json

    def load_json(path: Path):
        doc = real_load(path)
        if Path(path).resolve() == (HERE / "inventory.json").resolve():
            doc = dict(doc)
            doc["units"] = kept
        return doc

    V3RUN.load_json = load_json
    rc = V3RUN.main(argv)
    return rc


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
