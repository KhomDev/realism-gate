#!/usr/bin/env python3
"""Run a realism-gate binary over the solana-poc-v5 corpus (P2R-2 extension).

This is the v3 runner (`eval/solana-poc-v3/run.py`) with one change: unit
extraction understands v5's pin kinds and re-derives every unit from the pin
cache with the corpus's own builder (`build_inventory.py`), refusing any unit
whose hash differs from the sealed inventory.

- `report-page-block` (Code4rena Meteora final report, tier J): the cached
  report page is re-cut into finding sections exactly as the builder does, the
  content selector re-picks the block, and the block's normalised hash must
  match. The page itself is not hash-pinned (the live page is re-rendered);
  the block is.
- `repo-file` (Cashio reproduction, tier X; OtterSec Port Sundial, tier A):
  the git blob at the pinned commit is read with `git cat-file` (never the
  working tree) and its sha256 must equal `blob_sha256`. The file keeps its
  own name, so the binary sees its real extension.

Tier A units run but are reported apart; they never enter a J or X cell
(Amendment 2). The split guard is v3's: `--split` defaults to DEV, and a
sealed slice needs `--round`, `--version` and `--binary-sha256`. Never run
`--check` on a sealed split before its round: it extracts the units.

Usage:
  python -I eval/solana-poc-v5/run.py BINARY --version p2r2-dev --split DEV \
      --cache <worktree>/.corpus/solana-poc-v5 --out-dir <dir>
  python -I eval/solana-poc-v5/run.py --check --cache <cache>      (DEV only)
"""

from __future__ import annotations

import hashlib
import importlib.util
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
B = _load("p2r_v5_build", HERE / "build_inventory.py")

EXT = {"rust": "rs", "typescript": "ts", "javascript": "js"}
REPOS = {B.CASHIO_REPO: B.CASHIO_DIR, B.PORT_REPO: B.PORT_DIR}


def _meteora_code(unit: dict, pin: dict, cache: Path) -> str:
    page = cache / B.METEORA_HTML.relative_to(B.CACHE)
    if not page.is_file():
        raise SystemExit(f"{unit['id']}: report page missing from the cache")
    html = page.read_text(encoding="utf-8", errors="replace")
    want = unit["group_id"]
    for fid, title, lines in B.meteora_sections(html):
        if fid != want:
            continue
        md = "\n\n".join("```\n" + b + "\n```" for b in B._chunks(lines))
        sel = B.select_block(md, title)
        if sel is None or sel["block"] != pin["block"]:
            raise SystemExit(f"{unit['id']}: the selector no longer picks block {pin['block']}")
        if B.norm_hash(sel["code"]) != pin["block_sha256"]:
            raise SystemExit(f"{unit['id']}: block hash drift")
        return sel["code"]
    raise SystemExit(f"{unit['id']}: finding {want} missing from the report page")


def _repo_blob(unit: dict, pin: dict, cache: Path) -> bytes:
    local = REPOS.get(pin["repo"])
    if local is None:
        raise SystemExit(f"{unit['id']}: unknown repo {pin['repo']}")
    repo = cache / local.relative_to(B.CACHE)
    raw = B.git_blob(repo, pin["commit"], pin["path"])
    if hashlib.sha256(raw).hexdigest() != pin["blob_sha256"]:
        raise SystemExit(f"{unit['id']}: blob hash drift")
    return raw


def extract(unit: dict, cache: Path, out: Path) -> tuple[list[str], Path, bool]:
    pin = unit["files"][0]
    if pin["kind"] == "report-page-block":
        code = _meteora_code(unit, pin, cache)
        name = f"{unit['id'].lower()}.{EXT.get(pin.get('lang'), 'txt')}"
        (out / name).write_text(code, encoding="utf-8", newline="\n")
    elif pin["kind"] == "repo-file":
        raw = _repo_blob(unit, pin, cache)
        name = Path(pin["path"]).name
        (out / name).write_bytes(raw)
    else:
        raise SystemExit(f"{unit['id']}: unknown pin kind {pin['kind']}")
    return [name], out, unit.get("kind") == "test-fragment"


def main(argv: list[str]) -> int:
    V3RUN.HERE = HERE
    V3RUN.CACHE = ROOT / ".corpus" / "solana-poc-v5"
    V3RUN.SEED = "rg-solana-poc-v5-2026-11-27"
    V3RUN.extract = extract
    return V3RUN.main(argv)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
