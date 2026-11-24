#!/usr/bin/env python3
"""Runs a realism-gate binary over the pinned public Rust/Anchor TypeScript test
suites and writes every active finding (a "flag") as one JSON line.

The id is the first 16 hex digits of sha256(corpus|rule|file|line|column|evidence).
It leaves out the message and severity, so a label written against one version
still applies when a later version rewords the same flag. The evidence text is
not committed; the committed record carries its sha256, and `show.py` prints the
source around a flag for the reader who wants it.

The sample is at most 30 flags per rule per corpus, ordered by
sha256(seed + ':' + id), or every flag when the rule raised 30 or fewer. The
full flag list is committed; the sample file lists the ids that are labelled.

Usage:
  eval/solana/flags.py BINARY [--corpus NAME] [--out-dir DIR]
"""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solana"
WORK = ROOT / ".corpus" / "solana-work"
CAP = 30


def pins() -> dict:
    return json.loads((HERE / "corpus.json").read_text(encoding="utf-8"))


def sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def flag_id(corpus: str, f: dict) -> str:
    key = "|".join(
        [corpus, f["rule"], f["file"], str(f["line"]), str(f["column"]), f["evidence"]]
    )
    return sha256(key)[:16]


def language(path: str) -> str | None:
    if path.endswith(".rs"):
        return "rust"
    if path.endswith((".ts", ".tsx")):
        return "typescript"
    if path.endswith(".js"):
        # Anchor's older suites are JavaScript and use the same reader as the
        # TypeScript tests. The result is reported as a separate language, not
        # folded into TypeScript.
        return "javascript"
    return None


def included(path: str, patterns: list[str]) -> bool:
    return any(fnmatch.fnmatch(path, p) for p in patterns)


def run(binary: str, name: str, spec: dict) -> list[dict]:
    binary = str(Path(binary).resolve())
    base = Path(ROOT) / spec["dir"]
    args = [binary, "check", *spec["paths"], "--no-declaration", "--no-config",
            "--format", "json", "--fail-on", "never"]
    out = subprocess.run(args, cwd=base, capture_output=True)
    if out.returncode != 0:
        sys.stderr.write(out.stderr.decode("utf-8", "replace"))
        raise SystemExit(f"{name}: realism-gate exited {out.returncode}")
    report = json.loads(out.stdout.decode("utf-8"))
    flags = []
    for f in report.get("findings", []):
        if "suppressed" in f:
            continue
        if not included(f["file"], spec["include"]):
            continue
        lang = language(f["file"])
        if lang is None:
            continue
        flags.append(
            {
                "id": flag_id(name, f),
                "corpus": name,
                "rule": f["rule"],
                "slug": f.get("slug"),
                "severity": f.get("severity"),
                "language": lang,
                "file": f["file"],
                "line": f["line"],
                "column": f["column"],
                "suite": f.get("suite"),
                "tests": f.get("tests", []),
                "evidence_sha256": sha256(f["evidence"]),
            }
        )
    flags.sort(key=lambda x: (x["rule"], x["file"], x["line"], x["column"], x["id"]))
    ids = [f["id"] for f in flags]
    if len(ids) != len(set(ids)):
        raise SystemExit(f"{name}: duplicate flag ids")
    # Keep the evidence locally for the labeller; it is never committed.
    WORK.mkdir(parents=True, exist_ok=True)
    (WORK / f"{name}.findings.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n"
    )
    return flags


def sample(seed: str, flags: list[dict]) -> list[dict]:
    by_rule: dict[str, list[dict]] = {}
    for f in flags:
        by_rule.setdefault(f["rule"], []).append(f)
    picked = []
    for rule, rows in sorted(by_rule.items()):
        rows = sorted(rows, key=lambda f: sha256(seed + ":" + f["id"]))
        picked.extend(rows[:CAP])
    picked.sort(key=lambda x: (x["rule"], x["file"], x["line"], x["column"], x["id"]))
    return picked


def write_jsonl(path: Path, rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        for row in rows:
            fh.write(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n")


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("binary")
    p.add_argument("--corpus", action="append", choices=sorted(pins()["corpora"]))
    p.add_argument("--out-dir", default=str(HERE / "flags"))
    args = p.parse_args(argv[1:])
    pin = pins()
    names = args.corpus or sorted(pin["corpora"])
    out_dir = Path(args.out_dir)
    for name in names:
        spec = pin["corpora"][name]
        flags = run(args.binary, name, spec)
        write_jsonl(out_dir / f"v{pin['frozen_rule_version']}-{name}.jsonl", flags)
        picked = sample(pin["seed"], flags)
        write_jsonl(HERE / "samples" / f"{name}.jsonl", picked)
        by_rule: dict[str, int] = {}
        for f in flags:
            by_rule[f["rule"]] = by_rule.get(f["rule"], 0) + 1
        print(f"{name}: {len(flags)} flags, {len(picked)} sampled {dict(sorted(by_rule.items()))}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
