#!/usr/bin/env python3
"""Fetches and verifies every eval/solidity-poc-v1 pin.

Default: materialise missing checkouts/files in the gitignored cache
`.corpus/solidity-poc-v1/`, then verify. `--check`: verify only, no network.

Downloaded code is never executed; git is used only to check out and to read
HEAD, and files are read only for hashing.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solidity-poc-v1"
CACHE = ROOT / ".corpus" / "solidity-poc-v1"
INV = HERE / "inventory.json"

REPO_DIRS = {
    "https://github.com/SunWeb3Sec/DeFiHackLabs": "SunWeb3Sec-DeFiHackLabs",
    "https://github.com/immunefi-team/bugfix-reviews-pocs": "immunefi-team-bugfix-reviews-pocs",
    "https://github.com/faheelsattar/damn-vuln-defi-foundry": "faheelsattar-damn-vuln-defi-foundry",
    "https://github.com/catcatcat8/mixbytes-ctf": "catcatcat8-mixbytes-ctf",
}
SEC = re.compile(r'^## \[\[([HM])-(\d+)\]\s*(.*?)\]\((https://github\.com/[^)]+)\)(.*?)(?=^## \[\[|\n# |\Z)', re.S | re.M)
POCTEST = re.compile(r'function\s+test\w*\s*\(|function\s+invariant_\w*\s*\(|contract\s+\w*Test\w*\s*(?:is|\{)|vm\.(?:prank|deal|store|warp|startPrank)|assertEq\(|assertTrue\(|assertLt\(|assertGt\(')
FENCE = re.compile(r'```(?:solidity)?\n(.*?)```', re.S)
ALLFENCE = re.compile(r'```([A-Za-z0-9_+-]*)[ \t]*\n(.*?)```', re.S)
CODEHAWKS = CACHE / "codehawks"
HATS = CACHE / "hats"
UA = "independent-security-research (corpus pin verification)"


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


def sha_file(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def run(cmd: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")


def dir_for(repo: str) -> str:
    if repo in REPO_DIRS:
        return REPO_DIRS[repo]
    m = re.fullmatch(r"https://github\.com/code-423n4/(.+)", repo)
    if not m:
        raise SystemExit(f"unknown repo {repo}")
    slug = m.group(1)
    if slug.endswith("-findings"):
        return "c4-" + slug[: -len("-findings")]
    return "c4-code-" + slug if (CACHE / "repos" / ("c4-code-" + slug)).is_dir() else "c4-" + slug


def ensure_checkout(repo: str, commit: str) -> Path:
    dest = CACHE / "repos" / dir_for(repo)
    if dest.is_dir():
        head = run(["git", "-C", str(dest), "rev-parse", "HEAD"])
        if head.returncode == 0 and head.stdout.strip() == commit:
            return dest
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    run(["git", "-C", str(dest), "init", "-q"])
    run(["git", "-C", str(dest), "remote", "add", "origin", repo + ".git"])
    got = run(["git", "-C", str(dest), "fetch", "-q", "--depth", "1", "origin", commit])
    if got.returncode != 0:
        raise SystemExit(f"{repo}@{commit}: {got.stderr.strip()}")
    run(["git", "-C", str(dest), "checkout", "-q", "FETCH_HEAD"])
    return dest


def verify(units: list[dict]) -> list[str]:
    errs = []
    for u in units:
        f = u["files"][0]
        kind = f["kind"]
        if kind == "report-section-block":
            d = CACHE / "repos" / ("c4-" + u["source"])
            if not d.is_dir():
                errs.append(f"{u['id']}: missing findings checkout")
                continue
            head = run(["git", "-C", str(d), "rev-parse", "HEAD"])
            if head.stdout.strip() != f["commit"]:
                errs.append(f"{u['id']}: findings repo not at pin")
                continue
            rp = d / "report.md"
            if sha_file(rp) != f["report_sha256"]:
                errs.append(f"{u['id']}: report hash drift")
                continue
            text = rp.read_text(encoding="utf-8", errors="replace")
            sec = [m for m in SEC.finditer(text) if f"{m.group(1)}-{m.group(2)}" == f["section"]]
            if not sec:
                errs.append(f"{u['id']}: section {f['section']} missing")
                continue
            blocks = FENCE.findall(sec[0].group(5))
            if f["block"] >= len(blocks) or norm_hash(blocks[f["block"]]) != f["block_sha256"]:
                errs.append(f"{u['id']}: PoC block hash mismatch")
        elif kind == "repo-file":
            d = CACHE / "repos" / dir_for(f["repo"])
            p = d / f["path"]
            if not p.is_file():
                errs.append(f"{u['id']}: missing {f['path']}")
                continue
            head = run(["git", "-C", str(d), "rev-parse", "HEAD"])
            if head.stdout.strip() != f["commit"]:
                errs.append(f"{u['id']}: repo not at pin")
                continue
            if norm_hash(p.read_text(encoding="utf-8", errors="replace")) != f["sha256_norm"]:
                errs.append(f"{u['id']}: file hash mismatch")
        elif kind == "raw-file":
            p = CACHE / "raw" / "salty" / f["path"].replace("/", "_")
            if not p.is_file():
                errs.append(f"{u['id']}: missing raw file")
                continue
            if norm_hash(p.read_text(encoding="utf-8", errors="replace")) != f["sha256_norm"]:
                errs.append(f"{u['id']}: raw file hash mismatch")
        elif kind == "ch-page-json-block":
            slug = u["source"][len("codehawks-"):]
            p = CODEHAWKS / f"{slug}.html"
            if not p.is_file():
                errs.append(f"{u['id']}: missing CodeHawks page")
                continue
            if sha_file(p) != f["page_sha256"]:
                errs.append(f"{u['id']}: CodeHawks page hash drift")
                continue
            text = p.read_text(encoding="utf-8", errors="replace")
            i = text.find("findings.getFindingOverviewsForCompetition")
            k = text.find(">", i)
            e = text.find("</script>", k)
            res = json.loads(json.loads(text[k + 1:e])["body"])[0]["result"]["data"]
            groups = (res.get("highs") or []) + (res.get("mediums") or []) + (res.get("lows") or [])
            grp = next((g for g in groups if g["id"] == f["group"]), None)
            issue = next((x for x in (grp or {}).get("issues", []) if x["id"] == f["issue_id"]), None)
            if issue is None:
                errs.append(f"{u['id']}: CodeHawks submission not found")
                continue
            blocks = ALLFENCE.findall(issue.get("description") or "")
            if f["block"] >= len(blocks) or norm_hash(blocks[f["block"]][1]) != f["block_sha256"]:
                errs.append(f"{u['id']}: CodeHawks block hash mismatch")
        elif kind == "gh-issue-block":
            found = None
            for hp in sorted(HATS.glob("*.json")):
                for it in json.loads(hp.read_text(encoding="utf-8")):
                    if it["number"] == f["issue"]:
                        found = it
            if found is None:
                errs.append(f"{u['id']}: Hats issue not found")
                continue
            if hashlib.sha256((found.get("body") or "").encode("utf-8")).hexdigest() != f["body_sha256"]:
                errs.append(f"{u['id']}: Hats issue body drift")
                continue
            blocks = ALLFENCE.findall(found.get("body") or "")
            if f["block"] >= len(blocks) or norm_hash(blocks[f["block"]][1]) != f["block_sha256"]:
                errs.append(f"{u['id']}: Hats block hash mismatch")
        else:
            errs.append(f"{u['id']}: unknown pin kind {kind}")
    return errs


def fetch(units: list[dict]) -> None:
    seen = {}
    for u in units:
        f = u["files"][0]
        if f["kind"] in ("report-section-block", "repo-file"):
            seen.setdefault((f["repo"], f["commit"]), 0)
            seen[(f["repo"], f["commit"])] += 1
    for (repo, commit), n in sorted(seen.items()):
        ensure_checkout(repo, commit)
        print(f"repo: {repo}@{commit[:8]} ({n} units)")
    for u in units:
        f = u["files"][0]
        if f["kind"] == "ch-page-json-block":
            slug = u["source"][len("codehawks-"):]
            p = CODEHAWKS / f"{slug}.html"
            if not p.is_file() or sha_file(p) != f["page_sha256"]:
                p.parent.mkdir(parents=True, exist_ok=True)
                req = urllib.request.Request(f["url"], headers={"User-Agent": UA})
                with urllib.request.urlopen(req, timeout=180) as r:
                    data = r.read()
                if hashlib.sha256(data).hexdigest() != f["page_sha256"]:
                    raise SystemExit(f"{u['id']}: CodeHawks page sha mismatch")
                p.write_bytes(data)
                print(f"page: {slug}")
            continue
        if f["kind"] == "gh-issue-block":
            if not list(HATS.glob("*.json")):
                HATS.mkdir(parents=True, exist_ok=True)
                page = 1
                while True:
                    r = subprocess.run(["gh", "api", f"repos/{f['repo']}/issues?state=all&per_page=100&page={page}"],
                                       capture_output=True, text=True, encoding="utf-8", errors="replace")
                    if r.returncode != 0:
                        raise SystemExit(f"{f['repo']} issues page {page}: {r.stderr.strip()}")
                    (HATS / f"hats-euro-{page}.json").write_text(r.stdout, encoding="utf-8")
                    n = len(json.loads(r.stdout))
                    print(f"hats: page {page} ({n})")
                    if n < 100:
                        break
                    page += 1
                    time.sleep(8)
            continue
        if f["kind"] != "raw-file":
            continue
        p = CACHE / "raw" / "salty" / f["path"].replace("/", "_")
        if p.is_file() and norm_hash(p.read_text(encoding="utf-8", errors="replace")) == f["sha256_norm"]:
            continue
        p.parent.mkdir(parents=True, exist_ok=True)
        req = urllib.request.Request(f["url"], headers={"User-Agent": UA})
        with urllib.request.urlopen(req, timeout=120) as r:
            data = r.read()
        if norm_hash(data.decode("utf-8", "replace")) != f["sha256_norm"]:
            raise SystemExit(f"{u['id']}: raw sha mismatch")
        p.write_bytes(data)
        print(f"raw: {f['path']}")


def main(argv: list[str]) -> int:
    global CACHE, CODEHAWKS, HATS
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--cache", default=None,
                    help="pin cache root (default %(default)s); e.g. "
                         "worktrees/research-rg/.corpus/solidity-poc-v1")
    args = ap.parse_args(argv[1:])
    if args.cache:
        CACHE = Path(args.cache)
        CODEHAWKS = CACHE / "codehawks"
        HATS = CACHE / "hats"
    doc = json.loads(INV.read_text(encoding="utf-8"))
    units = doc["units"]
    if not args.check:
        fetch(units)
    errs = verify(units)
    if errs:
        print(f"{len(errs)} pin error(s):", file=sys.stderr)
        for e in errs[:30]:
            print("  " + e, file=sys.stderr)
        return 1
    print(f"all {len(units)} unit pins verified" + (" (offline)" if args.check else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
