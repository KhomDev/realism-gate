#!/usr/bin/env python3
"""Run a frozen realism-gate binary over every counted unit of the Solana v3 corpus.

The corpus stores pointers, not text. This script extracts each unit's block
from the pinned cache (`.corpus/solana-poc-v3/`) into a run directory, verifies
the block hash against the inventory, then runs the binary exactly as
`eval/solana-poc-v2/run.py` does, with two additions:

- fragments are passed with `--fragment`;
- a unit that names a protocol checkout (`target`) also passes
  `--target`/`--target-commit`, so the program model (RG-A1) can read it;
- the exploit model (RG-A2) gets `--model-target` with the source's checkout
  (SOURCE_TARGETS) when one exists in the pin cache. The binary must therefore
  be the P2 build that has `--model-target`; a pre-P2 binary rejects the flag.

Outputs (under `--out-dir`, default `<cache>/run-<version>`):
  runs.jsonl    one row per unit (readability, parse errors)
  flags.jsonl   every active finding (no evidence text, only its sha256)
  sample.jsonl  the flags to label: at most --cap per rule per source,
                ordered by sha256(seed + ':' + flag_id)
  extract/      the unit files the binary was given

Nothing here labels a flag or changes the corpus. `--check` re-extracts and
re-verifies every block hash without running the binary.

Usage:
  python eval/solana-poc-v3/run.py BINARY --version 1.5.0
  python eval/solana-poc-v3/run.py --check
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
CACHE = ROOT / ".corpus" / "solana-poc-v3"
SEED = "rg-solana-poc-v3-2026-11-25"
CAP = 30

FENCE = re.compile(r"```([A-Za-z0-9_+-]*)[ \t]*\n(.*?)```", re.S)
PRE = re.compile(r"<pre\b[^>]*>(.*?)</pre>", re.S)
TS_IMPORT = re.compile(
    r"""(?:\bfrom\s*|\bimport\s*\(?\s*|\brequire\s*\(\s*)["'](\.{1,2}/[^"']+)["']""")
RS_MOD = re.compile(r"""(?:#\[path\s*=\s*"([^"]+)"\]\s*)?(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;""")

KNOWN_REPOS = {
    "Frankcastleauditor/Solana-Audit-Arena": "Frankcastleauditor-Solana-Audit-Arena",
    "Zolldyk/anchor-pinochio-security-template": "Zolldyk-anchor-pinochio-security-template",
    "CodeHawks-Contests/2025-03-rustfund": "CodeHawks-Contests-2025-03-rustfund",
}

# The protocol checkout each source's tests run against, used as the exploit
# model's --model-target. RustFund and Zolldyk are single programs; the Arena
# repo holds several, so the model returns Unknown on an ambiguous instruction
# name rather than guessing.
SOURCE_TARGETS = {
    "arena": "Frankcastleauditor-Solana-Audit-Arena",
    "codehawks-rustfund": "CodeHawks-Contests-2025-03-rustfund",
    "rustfund": "CodeHawks-Contests-2025-03-rustfund",
    "zolldyk": "Zolldyk-anchor-pinochio-security-template",
}

EXT = {"rust": "rs", "typescript": "ts", "javascript": "js"}


def norm_text(text: str) -> str:
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    lines = [line.rstrip() for line in text.split("\n")]
    while lines and not lines[0]:
        lines.pop(0)
    while lines and not lines[-1]:
        lines.pop()
    return "\n".join(lines)


def norm_hash(text: str) -> str:
    return hashlib.sha256(norm_text(text).encode("utf-8")).hexdigest()


def sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def load_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def infer_lang(name: str) -> str | None:
    if name.endswith(".rs"):
        return "rust"
    if name.endswith((".ts", ".tsx", ".mts", ".cts")):
        return "typescript"
    if name.endswith((".js", ".mjs", ".cjs")):
        return "javascript"
    return None


def unit_lang(unit: dict) -> str:
    f = unit["files"][0]
    if f.get("kind") == "repo-file":
        return infer_lang(f["path"]) or "other"
    return {"rust": "rust", "typescript": "typescript", "javascript": "javascript"}.get(
        f.get("lang"), "other")


def extract(unit: dict, cache: Path, out: Path) -> tuple[list[str], Path, bool]:
    """Write the unit's files under `out`; return (relative files, base, fragment)."""
    f = unit["files"][0]
    kind = f["kind"]
    fragment = unit.get("kind") == "test-fragment"
    if kind == "issue-block":
        issues = {i["number"]: i for i in load_json(cache / "arena" / "issues.json")}
        body = issues[f["issue"]].get("body") or ""
        blocks = [m for m in FENCE.finditer(body)]
        if f["block"] >= len(blocks):
            raise SystemExit(f"{unit['id']}: block {f['block']} missing")
        code = blocks[f["block"]].group(2)
        if norm_hash(code) != f["block_sha256"]:
            raise SystemExit(f"{unit['id']}: block hash drift")
        name = f"{unit['id'].lower()}.{EXT.get(unit_lang(unit), 'txt')}"
        (out / name).write_text(code, encoding="utf-8", newline="\n")
        return [name], out, fragment
    if kind == "page-json-block":
        page = (cache / "pages" / "rustfund-results.html").read_bytes().decode(
            "utf-8", "replace")
        i = page.find("findings.getFindingOverviewsForCompetition")
        k = page.find(">", i)
        e = page.find("</script>", k)
        res = json.loads(json.loads(page[k + 1:e])["body"])[0]["result"]["data"]
        groups = ((res.get("highs") or []) + (res.get("mediums") or [])
                  + (res.get("lows") or []) + (res.get("gas") or []))
        group = next((g for g in groups if g["id"] == f["group"]), None)
        issue = next((x for x in (group or {}).get("issues", [])
                      if x["id"] == f["issue_id"]), None)
        if issue is None:
            raise SystemExit(f"{unit['id']}: issue {f['issue_id']} missing")
        blocks = [m for m in FENCE.finditer(issue.get("description") or "")]
        if f["block"] >= len(blocks):
            raise SystemExit(f"{unit['id']}: block {f['block']} missing")
        code = blocks[f["block"]].group(2)
        if norm_hash(code) != f["block_sha256"]:
            raise SystemExit(f"{unit['id']}: block hash drift")
        name = f"{unit['id'].lower()}.{EXT.get(unit_lang(unit), 'txt')}"
        (out / name).write_text(code, encoding="utf-8", newline="\n")
        return [name], out, fragment
    if kind == "page-pre-block":
        import html as html_mod
        page = (cache / "pages" / "jupiter-lend-report.html").read_bytes().decode(
            "utf-8", "replace")
        pres = [m for m in PRE.finditer(page)]
        inner = html_mod.unescape(re.sub(r"<[^>]+>", "", pres[f["pre_index"]].group(1)))
        if norm_hash(inner) != f["block_sha256"]:
            raise SystemExit(f"{unit['id']}: pre block hash drift")
        name = f"{unit['id'].lower()}.rs"
        (out / name).write_text(inner, encoding="utf-8", newline="\n")
        return [name], out, False
    if kind == "repo-file":
        dirname = KNOWN_REPOS.get(f["repo"])
        if dirname is None:
            raise SystemExit(f"{unit['id']}: unknown repo {f['repo']}")
        base = cache / "repos" / dirname
        p = base / f["path"]
        if not p.is_file():
            raise SystemExit(f"{unit['id']}: missing {f['path']}")
        if norm_hash(p.read_text(encoding="utf-8", errors="replace")) != f["sha256_norm"]:
            raise SystemExit(f"{unit['id']}: file hash drift")
        return [f["path"]], base, False
    raise SystemExit(f"{unit['id']}: unknown pin kind {kind}")


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
        lang = infer_lang(rel)
        if lang in ("typescript", "javascript"):
            for spec in TS_IMPORT.findall(text):
                for ext in ("", ".ts", ".tsx", ".js", ".mjs", ".cjs", "/index.ts", "/index.js"):
                    cand = (here / (spec + ext)).resolve()
                    if cand.is_file() and infer_lang(str(cand)):
                        found.append(cand)
                        break
        elif rel.endswith(".rs"):
            moddir = here if p.name in ("mod.rs", "lib.rs", "main.rs") else here / p.stem
            for path_attr, name in RS_MOD.findall(text):
                cands = ([here / path_attr] if path_attr else
                         [moddir / f"{name}.rs", moddir / name / "mod.rs",
                          here / f"{name}.rs", here / name / "mod.rs"])
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


def run_unit(binary: str, unit: dict, cache: Path, extract_root: Path) -> tuple[dict, list[dict]]:
    out = extract_root / unit["id"]
    out.mkdir(parents=True, exist_ok=True)
    files, base, fragment = extract(unit, cache, out)
    files = closure(base, files)
    args = [binary, "check", *files, "--no-declaration", "--no-config",
            "--format", "json", "--fail-on", "never"]
    if fragment:
        args.append("--fragment")
    target = unit.get("target")
    if target:
        args += ["--target", target["path"], "--target-commit", target["commit"]]
    model_target = target
    if model_target is None and unit.get("source") in SOURCE_TARGETS:
        checkout = cache / "repos" / SOURCE_TARGETS[unit["source"]]
        if checkout.is_dir():
            model_target = {"path": str(checkout)}
    if model_target:
        # The protocol checkout feeds the exploit model (RG-A2). It is separate
        # from --target/--target-commit, which drive RG20 citation checks.
        args += ["--model-target", model_target["path"]]
    proc = subprocess.run(args, cwd=base, capture_output=True)
    no_suite = proc.returncode == 2 and b"no test suite found" in proc.stderr
    if proc.returncode != 0 and not no_suite:
        sys.stderr.write(proc.stderr.decode("utf-8", "replace"))
        raise SystemExit(f"{unit['id']}: realism-gate exited {proc.returncode}")
    report = json.loads(proc.stdout.decode("utf-8"))
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
            "split": unit.get("split"),
            "rule": f["rule"],
            "slug": f.get("slug"),
            "severity": f.get("severity"),
            "language": infer_lang(f["file"]) or "other",
            "file": f["file"],
            "line": f["line"],
            "column": f["column"],
            "suite": f.get("suite"),
            "tests": f.get("tests", []),
            # The message names what the rule claims at this span. A label
            # reader needs it; without it the reader sees the pattern but not
            # which actor or step the finding is about. Evidence text stays
            # out (third-party source; pointer-plus-hash only).
            "message": f.get("message", ""),
            "evidence_sha256": sha256(f["evidence"]),
        })
    flags.sort(key=lambda x: (x["rule"], x["file"], x["line"], x["column"], x["id"]))
    row = {
        "unit": unit["id"],
        "source": unit["source"],
        "tier": unit["tier"],
        "split": unit.get("split"),
        "kind": unit["kind"],
        "language": unit_lang(unit),
        "files_given": files,
        "files_read": summary["files"],
        "tests_checked": summary["tests"],
        "suites_checked": summary["suites"],
        "parse_errors": len(report.get("parse_errors", [])),
        "readable": summary["tests"] > 0,
        "active_flags": len(flags),
    }
    return row, flags


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("binary", nargs="?")
    ap.add_argument("--version", default="unfrozen")
    ap.add_argument("--cache", default=str(CACHE))
    ap.add_argument("--out-dir", default=None)
    ap.add_argument("--split", default="DEV",
                    help="comma-separated slices to touch (default DEV). V1/V2/FINAL "
                         "are opened only by a frozen round run.")
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args(argv[1:])
    cache = Path(args.cache).resolve()
    inv = load_json(HERE / "inventory.json")
    wanted = {s.strip().upper() for s in args.split.split(",") if s.strip()}
    units = [u for u in inv["units"] if (u.get("split") or "").upper() in wanted]
    if not units:
        raise SystemExit(f"no units in split {sorted(wanted)}")
    if args.check:
        bad = 0
        tmp = cache / "extract-check"
        for unit in units:
            out = tmp / unit["id"]
            out.mkdir(parents=True, exist_ok=True)
            try:
                extract(unit, cache, out)
            except SystemExit as e:
                print(e, file=sys.stderr)
                bad += 1
        print(f"{len(units)} units extracted, {bad} hash or pin error(s)")
        return 1 if bad else 0
    if not args.binary:
        ap.error("BINARY is required unless --check")
    binary = str(Path(args.binary).resolve())
    out_dir = (Path(args.out_dir) if args.out_dir else cache / f"run-{args.version}").resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    rows, flags = [], []
    for unit in units:
        row, fl = run_unit(binary, unit, cache, out_dir / "extract")
        rows.append(row)
        flags.extend(fl)
    ids = [f["id"] for f in flags]
    if len(ids) != len(set(ids)):
        raise SystemExit("duplicate flag ids")
    write_jsonl(out_dir / "runs.jsonl", rows)
    write_jsonl(out_dir / "flags.jsonl", flags)
    poc = [f for f in flags if f["tier"] in ("J", "X")]
    write_jsonl(out_dir / "sample.jsonl", sample(SEED, poc))
    readable = sum(r["readable"] for r in rows)
    print(f"{len(rows)} units, {readable} readable, {len(flags)} flags, "
          f"{len(sample(SEED, poc))} sampled -> {out_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
