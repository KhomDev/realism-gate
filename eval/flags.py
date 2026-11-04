#!/usr/bin/env python3
"""Runs a realism-gate binary over one pinned corpus and writes every active
finding (a "flag") as one JSON line, with a stable id.

The id is the first 16 hex digits of the sha256 of rule, file, line, column
and evidence. It leaves out the message and the severity, so a label written
against one version still applies when a later version rewords the message or
changes the severity of the same flag.

For a corpus without a stated licence the evidence text is left out of the
output (it is still part of the id).

Usage:
  flags.py BINARY CORPUS OUT.jsonl      CORPUS is a name from eval/corpus.json
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "eval"))
import corpus  # noqa: E402


def flag_id(f: dict) -> str:
    key = "|".join([f["rule"], f["file"], str(f["line"]), str(f["column"]), f["evidence"]])
    return hashlib.sha256(key.encode("utf-8")).hexdigest()[:16]


def run(binary: str, name: str) -> list[dict]:
    pins = corpus.pins()[name]
    keep = set(corpus.files(name))
    with_text = pins["license"] in ("Apache-2.0", "MIT")
    base = [binary, "check", pins["path"], "--no-declaration", "--format", "json", "--fail-on", "never"]
    out = subprocess.run(base + ["--no-config"], cwd=corpus.directory(name), capture_output=True)
    if out.returncode != 0:
        # 0.1.0 has no --no-config; it reads no config file either.
        out = subprocess.run(base, cwd=corpus.directory(name), capture_output=True, check=True)
    report = json.loads(out.stdout.decode("utf-8"))
    flags = []
    for f in report["findings"]:
        if "suppressed" in f or f["file"] not in keep:
            continue
        flag = {
            "id": flag_id(f),
            "rule": f["rule"],
            "severity": f["severity"],
            "file": f["file"],
            "line": f["line"],
            "column": f["column"],
            "suite": f["suite"],
            "tests": f["tests"],
        }
        if with_text:
            flag["evidence"] = f["evidence"]
        flags.append(flag)
    flags.sort(key=lambda x: (x["rule"], x["file"], x["line"], x["column"]))
    return flags


def main(argv: list[str]) -> int:
    if len(argv) != 4 or argv[2] not in corpus.pins():
        print(__doc__, file=sys.stderr)
        return 2
    flags = run(str(Path(argv[1]).resolve()), argv[2])
    ids = [f["id"] for f in flags]
    assert len(ids) == len(set(ids)), "flag ids must be unique"
    with open(argv[3], "w", encoding="utf-8", newline="\n") as fh:
        for f in flags:
            fh.write(json.dumps(f, ensure_ascii=False) + "\n")
    by_rule: dict[str, int] = {}
    for f in flags:
        by_rule[f["rule"]] = by_rule.get(f["rule"], 0) + 1
    print(f"{argv[3]}: {len(flags)} flags {dict(sorted(by_rule.items()))}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
