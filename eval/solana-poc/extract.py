#!/usr/bin/env python3
"""Extracts the PoCs that live in a judged issue (markdown) or a public report
page (HTML) into a gitignored work tree, so the tool can read them as files.

Extraction rule (frozen in PREREGISTRATION.md):
- The fenced code block the inventory names (by index, in document order) is
  written verbatim, with line endings normalised to LF. Nothing is wrapped,
  edited, completed or given imports.
- When the PoC is meant to sit in the code repository (a host directory is
  named), that directory is first copied from the pinned code checkout, so
  relative imports resolve the way they would for the warden; the PoC is then
  written into it under the inventory's name (replacing a file of that name
  when the warden said "replace").
- The extracted text is never committed. inventory.json records its sha256.

Usage:
  eval/solana-poc/extract.py
"""

from __future__ import annotations

import html
import json
import re
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import inventory  # noqa: E402

ROOT = inventory.ROOT
WORK = inventory.WORK


def md_blocks(text: str) -> list[str]:
    return [m.group(2) for m in re.finditer(r"```([^\n]*)\n(.*?)```", text, re.S)]


def report_blocks(page: str, anchor: str) -> list[str]:
    parts = re.split(r'<h2 id="([^"]+)"', page)
    for i in range(1, len(parts), 2):
        if parts[i] == anchor:
            out = []
            for c in re.findall(r"<pre[^>]*>(.*?)</pre>", parts[i + 1], re.S):
                out.append(html.unescape(re.sub(r"<[^>]+>", "", c)))
            return out
    raise SystemExit(f"report section {anchor} not found")


def main() -> int:
    corpus = inventory.pins()
    n = 0
    for u in inventory.all_units():
        for f in u["files"]:
            if "extract" not in f:
                continue
            dest_root = WORK / "extract" / u["id"]
            if dest_root.exists():
                shutil.rmtree(dest_root)
            dest_dir = dest_root / f.get("host_dir", "")
            if f.get("host_source"):
                src = ROOT / corpus["corpora"][f["host_source"]]["dir"] / f["host_dir"]
                shutil.copytree(src, dest_dir)
            dest_dir.mkdir(parents=True, exist_ok=True)
            if f["extract"].startswith("report:"):
                rep = corpus["reports"][f["extract"].split(":", 1)[1]]
                page = (ROOT / rep["file"]).read_text(encoding="utf-8")
                blocks = report_blocks(page, f["anchor"])
            else:
                md = ROOT / corpus["corpora"][f["extract"]]["dir"] / f["md"]
                blocks = md_blocks(md.read_text(encoding="utf-8"))
            text = blocks[f["block"]].replace("\r\n", "\n")
            (dest_dir / f["name"]).write_text(text, encoding="utf-8", newline="\n")
            n += 1
    print(f"extracted {n} PoC blocks into {WORK.relative_to(ROOT)}/extract")
    return 0


if __name__ == "__main__":
    sys.exit(main())
