#!/usr/bin/env python3
"""Runs a realism-gate binary over every counted PoC unit of the inventory, one
unit at a time, and writes:

- runs/v<version>.jsonl  one row per unit: the files the tool was given, how
  many tests it checked (`summary.tests`), parse errors, and whether the unit
  is readable (RQ1);
- flags/v<version>.jsonl every active finding raised on a unit (no evidence
  text, only its sha256);
- sample.jsonl           the flags to label: at most 30 per rule per source,
  ordered by sha256(seed + ':' + flag_id).

What the tool is given (frozen in PREREGISTRATION.md): the unit's files plus
every local file they import, transitively. TypeScript/JavaScript: relative
`import ... from './x'`, `export ... from './x'`, `import './x'` and
`require('./x')`, resolved to x, x.ts, x.tsx, x.js, x.mjs, x.cjs, x/index.ts,
x/index.js. Rust: `mod x;` resolved to x.rs or x/mod.rs beside the file (or
inside the directory named after a non-mod.rs file), and `#[path = "..."]`.
Nothing outside the unit's checkout or work directory is added.

The flag id is the first 16 hex digits of
sha256(unit|rule|file|line|column|evidence), as in eval/solana/flags.py with
the unit in place of the corpus.

Usage:
  eval/solana-poc/run.py BINARY
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import inventory  # noqa: E402

ROOT = inventory.ROOT
HERE = inventory.HERE
WORK = inventory.WORK
CAP = 30

TS_EXT = ("", ".ts", ".tsx", ".js", ".mjs", ".cjs", "/index.ts", "/index.js")
TS_IMPORT = re.compile(
    r"""(?:\bfrom\s*|\bimport\s*\(?\s*|\brequire\s*\(\s*)["'](\.{1,2}/[^"']+)["']""")
RS_MOD = re.compile(r"""(?:#\[path\s*=\s*"([^"]+)"\]\s*)?(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;""")


def sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def language(path: str) -> str | None:
    if path.endswith(".rs"):
        return "rust"
    if path.endswith((".ts", ".tsx", ".mts", ".cts")):
        return "typescript"
    if path.endswith((".js", ".mjs", ".cjs")):
        return "javascript"
    return None


def base_dir(unit: dict, corpus: dict) -> Path:
    if any("extract" in f for f in unit["files"]):
        return WORK / "extract" / unit["id"]
    return ROOT / corpus["corpora"][unit["source"]]["dir"]


def rel_files(unit: dict) -> list[str]:
    out = []
    for f in unit["files"]:
        if "extract" in f:
            out.append(str(Path(f.get("host_dir", "")) / f["name"]).replace("\\", "/").lstrip("/"))
        else:
            out.append(f["path"])
    return out


def closure(base: Path, start: list[str]) -> list[str]:
    seen: list[str] = []
    todo = list(start)
    while todo:
        rel = todo.pop(0)
        if rel in seen:
            continue
        p = base / rel
        if not p.is_file():
            continue
        seen.append(rel)
        text = p.read_text(encoding="utf-8", errors="replace")
        here = p.parent
        found: list[Path] = []
        if language(rel) in ("typescript", "javascript"):
            for spec in TS_IMPORT.findall(text):
                for ext in TS_EXT:
                    cand = (here / (spec + ext)).resolve()
                    if cand.is_file() and language(str(cand)):
                        found.append(cand)
                        break
        elif rel.endswith(".rs"):
            moddir = here if p.name in ("mod.rs", "lib.rs", "main.rs") else here / p.stem
            for path_attr, name in RS_MOD.findall(text):
                if path_attr:
                    cands = [here / path_attr]
                else:
                    cands = [moddir / f"{name}.rs", moddir / name / "mod.rs",
                             here / f"{name}.rs", here / name / "mod.rs"]
                for cand in cands:
                    if cand.is_file():
                        found.append(cand.resolve())
                        break
        for cand in found:
            try:
                r = cand.relative_to(base.resolve()).as_posix()
            except ValueError:
                continue
            if r not in seen:
                todo.append(r)
    return seen


def flag_id(unit: str, f: dict) -> str:
    key = "|".join([unit, f["rule"], f["file"], str(f["line"]), str(f["column"]), f["evidence"]])
    return sha256(key)[:16]


def run_unit(binary: str, unit: dict, corpus: dict) -> tuple[dict, list[dict], dict]:
    base = base_dir(unit, corpus)
    files = closure(base, rel_files(unit))
    args = [binary, "check", *files, "--no-declaration", "--no-config",
            "--format", "json", "--fail-on", "never"]
    out = subprocess.run(args, cwd=base, capture_output=True)
    if out.returncode != 0:
        sys.stderr.write(out.stderr.decode("utf-8", "replace"))
        raise SystemExit(f"{unit['id']}: realism-gate exited {out.returncode}")
    report = json.loads(out.stdout.decode("utf-8"))
    summary = report["summary"]
    flags = []
    for f in report.get("findings", []):
        if "suppressed" in f:
            continue
        flags.append({
            "id": flag_id(unit["id"], f),
            "unit": unit["id"],
            "source": unit["source"],
            "tier": unit["tier"],
            "rule": f["rule"],
            "slug": f.get("slug"),
            "severity": f.get("severity"),
            "language": language(f["file"]) or "other",
            "file": f["file"],
            "line": f["line"],
            "column": f["column"],
            "suite": f.get("suite"),
            "tests": f.get("tests", []),
            "evidence_sha256": sha256(f["evidence"]),
        })
    flags.sort(key=lambda x: (x["rule"], x["file"], x["line"], x["column"], x["id"]))
    row = {
        "unit": unit["id"],
        "source": unit["source"],
        "tier": unit["tier"],
        "kind": unit["kind"],
        "language": language(rel_files(unit)[0]) or "other",
        "files_given": files,
        "tests_checked": summary["tests"],
        "suites_checked": summary["suites"],
        "parse_errors": len(report.get("parse_errors", [])),
        "readable": summary["tests"] > 0,
        "active_flags": len(flags),
    }
    return row, flags, report


def sample(seed: str, flags: list[dict]) -> list[dict]:
    groups: dict[tuple[str, str], list[dict]] = {}
    for f in flags:
        groups.setdefault((f["source"], f["rule"]), []).append(f)
    picked = []
    for _, rows in sorted(groups.items()):
        rows = sorted(rows, key=lambda f: sha256(seed + ":" + f["id"]))
        picked.extend(rows[:CAP])
    picked.sort(key=lambda x: (x["rule"], x["unit"], x["file"], x["line"], x["column"], x["id"]))
    return picked


def write_jsonl(path: Path, rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        for row in rows:
            fh.write(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n")


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("binary")
    args = p.parse_args(argv[1:])
    binary = str(Path(args.binary).resolve())
    corpus = inventory.pins()
    inv = json.loads((HERE / "inventory.json").read_text(encoding="utf-8"))
    version = corpus["frozen_rule_version"]
    rows, flags = [], []
    reports = {}
    for unit in inv["units"]:
        if "duplicate_of" in unit:
            continue
        row, fl, report = run_unit(binary, unit, corpus)
        rows.append(row)
        flags.extend(fl)
        reports[unit["id"]] = report
    ids = [f["id"] for f in flags]
    if len(ids) != len(set(ids)):
        raise SystemExit("duplicate flag ids")
    write_jsonl(HERE / "runs" / f"v{version}.jsonl", rows)
    write_jsonl(HERE / "flags" / f"v{version}.jsonl", flags)
    # Only PoC tiers are sampled for labels; negative controls are reported by count.
    poc_flags = [f for f in flags if f["tier"] in ("J", "X")]
    write_jsonl(HERE / "sample.jsonl", sample(corpus["seed"], poc_flags))
    # Evidence stays local for the label reader; never committed.
    WORK.mkdir(parents=True, exist_ok=True)
    (WORK / "findings.json").write_text(json.dumps(reports, indent=1) + "\n", encoding="utf-8", newline="\n")
    readable = sum(r["readable"] for r in rows)
    print(f"{len(rows)} units, {readable} readable, {len(flags)} flags "
          f"({len(poc_flags)} on PoCs), {len(sample(corpus['seed'], poc_flags))} sampled")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
