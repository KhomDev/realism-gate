#!/usr/bin/env python3
"""Materialise the Solidity v2 audited codebases (Sherlock contest snapshots).

`inventory.json` names a repo and commit per `codebase_id`. This clones each
missing one at its pinned commit into `<cache>/repos/<dir>`, the directory the
v1 runner's `codebase_dir` resolves, so `--model-target` can read it. It reuses
the v1 fetcher's `clone` (blob-less clone, depth-1 fetch of the pinned commit,
checkout, HEAD check). Network only; downloaded code is never executed. A repo
that cannot be fetched is recorded and left absent; the runner then reports
`checkout-absent` for its units. Reading a codebase reads no unit's PoC code.

Usage:
  python -I eval/solidity-poc-v2/fetch_codebases.py --cache <cache> [--only ID]
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
V1_DIR = ROOT / "eval" / "solidity-poc-v1"
sys.path.insert(0, str(V1_DIR))


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


V1FETCH = _load("p2r_sol_v1_fetch_codebases", V1_DIR / "fetch_codebases.py")
V1RUN = sys.modules["run"]


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cache", default=str(ROOT / ".corpus" / "solidity-poc-v2"))
    ap.add_argument("--only", action="append", default=[])
    args = ap.parse_args(argv[1:])
    cache = Path(args.cache).resolve()
    inventory = json.loads((HERE / "inventory.json").read_text(encoding="utf-8"))
    used = {u.get("codebase_id") for u in inventory["units"] if u.get("codebase_id")}
    results: dict[str, str] = {}
    for cid in sorted(used):
        if args.only and cid not in args.only:
            continue
        cb = inventory["codebases"].get(cid)
        if not cb or not cb.get("commit"):
            results[cid] = "no-pin"
            continue
        dest = cache / "repos" / V1RUN.codebase_dir(cb["repo"], cache)
        results[cid] = V1FETCH.clone(cb["repo"], cb["commit"], dest)
        print(f"{cid}: {results[cid]}", flush=True)
    summary: dict[str, int] = {}
    for status in results.values():
        key = status.split(":", 1)[0]
        summary[key] = summary.get(key, 0) + 1
    print(json.dumps(summary, sort_keys=True))
    (cache / "repos").mkdir(parents=True, exist_ok=True)
    (cache / "repos" / "_fetch_codebases.json").write_text(
        json.dumps(results, indent=1, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
