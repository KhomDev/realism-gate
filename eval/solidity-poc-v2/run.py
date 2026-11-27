#!/usr/bin/env python3
"""Run a realism-gate binary over the solidity-poc-v2 corpus (P2R-2 additions).

This is the v1 runner (`eval/solidity-poc-v1/run.py`: fragment wrapping,
codebase copies, `--model-target`, the split guard, sampling) with one change:
unit extraction understands v2's pin kinds and re-derives every block from the
pin cache with the corpus's own builder, refusing any drift.

- `sherlock-report-section-block`: the judging repo's `README.md` is read as the
  **git blob** at the pinned commit (never the working tree: `core.autocrlf`
  would change the bytes), hashed against `report_sha256`, the `Issue H-n` /
  `M-n` section re-found, and the content selector must re-pick the pinned
  block with the pinned hash. Runs like a Code4rena report block.
- `page-pre-block` (bounty write-up page): the cached page is converted as the
  builder does and the selected block must match `block_sha256`; the page
  bytes are dynamic and are not a pin.
- `raw-file` (bounty PoC file linked from a write-up) and `repo-file`
  (coinspect/learn-evm-attacks, tier X): normalised file hash against
  `sha256_norm`.

Sherlock codebase snapshots are materialised by `fetch_codebases.py` (this
directory) under `<cache>/repos/<slug>` so the program model can read them.

Usage:
  python -I eval/solidity-poc-v2/run.py BINARY --version p2r2-dev --split DEV \
      --cache <worktree>/.corpus/solidity-poc-v2 --out-dir <dir>
  python -I eval/solidity-poc-v2/run.py --check --cache <cache>
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
V1_DIR = ROOT / "eval" / "solidity-poc-v1"


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


# v1's runner imports its own `build_inventory` by module name from its folder.
sys.path.insert(0, str(V1_DIR))
V1RUN = _load("p2r_sol_v1_run", V1_DIR / "run.py")
B = _load("p2r_sol_v2_build", HERE / "build_inventory.py")
PinError = V1RUN.PinError
WRITEUP_CACHE = {w["id"]: w["pin"]["cache"] for w in B.WRITEUPS}


def git_blob(repo: Path, commit: str, path: str) -> bytes:
    proc = subprocess.run(["git", "-C", str(repo), "cat-file", "blob", f"{commit}:{path}"],
                          capture_output=True)
    if proc.returncode != 0:
        raise PinError(f"no blob {commit[:12]}:{path} in {repo.name}")
    return proc.stdout


def sherlock_block(unit: dict, pin: dict, cache: Path) -> str:
    name = pin["repo"].rstrip("/").rsplit("/", 1)[-1]
    repo = cache / "sherlock" / name
    if not repo.is_dir():
        raise PinError(f"missing judging checkout {name}")
    blob = git_blob(repo, pin["commit"], pin["path"])
    if hashlib.sha256(blob).hexdigest() != pin["report_sha256"]:
        raise PinError("report blob hash drift")
    text = blob.decode("utf-8", errors="replace").replace("\r\n", "\n").replace("\r", "\n")
    for m in B.SHER_SEC.finditer(text):
        if f"{m.group(1)}-{m.group(2)}" != pin["section"]:
            continue
        body = m.group(4).split("\n## Discussion", 1)[0]
        sel = B.select_sol_block(body)
        if sel is None or sel["block"] != pin["block"]:
            raise PinError(f"selector no longer picks block {pin['block']}")
        if V1RUN.norm_hash(sel["code"]) != pin["block_sha256"]:
            raise PinError("PoC block hash mismatch")
        return sel["code"]
    raise PinError(f"section {pin['section']} missing")


def writeup_block(unit: dict, pin: dict, cache: Path) -> str:
    rel = WRITEUP_CACHE.get(unit["id"])
    if rel is None:
        raise PinError("write-up unit has no cache entry in the builder")
    raw = (cache / rel).read_text(encoding="utf-8", errors="replace")
    sel = B.select_sol_block(B.page_markdown(raw))
    if sel is None or sel["block"] != pin["block"]:
        raise PinError("selector no longer picks the pinned block")
    if V1RUN.norm_hash(sel["code"]) != pin["block_sha256"]:
        raise PinError("write-up block hash mismatch")
    return sel["code"]


def extract_code(unit: dict, cache: Path) -> dict:
    pin = unit["files"][0]
    kind = pin["kind"]
    if kind == "sherlock-report-section-block":
        # Runs exactly like a Code4rena report block (wrapped, codebase copy).
        return {"pin_kind": "report-section-block", "code": sherlock_block(unit, pin, cache)}
    if kind == "page-pre-block":
        return {"pin_kind": "report-section-block", "code": writeup_block(unit, pin, cache)}
    if kind == "raw-file":
        rel = WRITEUP_CACHE.get(unit["id"])
        if rel is None:
            raise PinError("raw-file unit has no cache entry in the builder")
        text = (cache / rel).read_text(encoding="utf-8", errors="replace")
        if V1RUN.norm_hash(text) != pin["sha256_norm"]:
            raise PinError("raw file hash mismatch")
        return {"pin_kind": "raw-file", "code": text, "raw_rel": Path(pin["path"]).name}
    if kind == "repo-file":
        base = cache / "x" / "learn-evm-attacks"
        if not base.is_dir():
            raise PinError("missing learn-evm-attacks checkout")
        if V1RUN.git_head(base) != pin["commit"]:
            raise PinError("learn-evm-attacks HEAD drift")
        text = git_blob(base, pin["commit"], pin["path"]).decode("utf-8", errors="replace")
        if V1RUN.norm_hash(text) != pin["sha256_norm"]:
            raise PinError("file hash mismatch")
        return {"pin_kind": "repo-file", "code": text, "repo_base": base, "repo_rel": pin["path"]}
    raise PinError(f"unknown pin kind {kind}")


def main(argv: list[str]) -> int:
    V1RUN.HERE = HERE
    V1RUN.DEFAULT_CACHE = ROOT / ".corpus" / "solidity-poc-v2"
    V1RUN.SEED = "rg-solidity-poc-v2-2026-11-27"
    V1RUN.extract_code = extract_code
    rc = V1RUN.main(argv)
    # v1's meta names its own corpus; correct it for this run.
    out = None
    for i, a in enumerate(argv):
        if a == "--out-dir" and i + 1 < len(argv):
            out = Path(argv[i + 1])
    if out is not None and (out / "meta.json").is_file():
        meta = json.loads((out / "meta.json").read_text(encoding="utf-8"))
        meta["corpus"] = "solidity-poc-v2"
        (out / "meta.json").write_text(json.dumps(meta, indent=2, sort_keys=True) + "\n",
                                       encoding="utf-8", newline="\n")
    return rc


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
