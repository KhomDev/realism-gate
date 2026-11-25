#!/usr/bin/env python3
"""Fetches and verifies every pin of eval/solana-poc-v3.

Default: download anything missing into the gitignored cache
`.corpus/solana-poc-v3/`, then verify every unit against `inventory.json`.
`--check`: verify only; makes no network call.

Nothing downloaded is ever executed. Repository checkouts are only read by
`git` and by hashing; `git checkout` writes files but runs no program code.

Fetches that need authentication: the Solana Audit Arena issues use `gh api`
(the operator's own GitHub auth). Four calls, 8 s apart, within the 10/min
budget. Everything else is a plain HTTPS GET.

Cache layout:
  arena/issues.json          derived from the four issue-list pages
  pages/rustfund-results.html    Wayback snapshot of the CodeHawks results
  pages/jupiter-lend-report.html Code4rena public report
  repos/<owner>-<repo>/          git checkouts at the pinned commit
"""

from __future__ import annotations

import argparse
import hashlib
import html as html_mod
import json
import re
import shutil
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solana-poc-v3"
CACHE = ROOT / ".corpus" / "solana-poc-v3"
INV = HERE / "inventory.json"

KNOWN_REPOS = {
    "Frankcastleauditor/Solana-Audit-Arena": "Frankcastleauditor-Solana-Audit-Arena",
    "Zolldyk/anchor-pinochio-security-template": "Zolldyk-anchor-pinochio-security-template",
    "CodeHawks-Contests/2025-03-rustfund": "CodeHawks-Contests-2025-03-rustfund",
}

UA = "independent-security-research (corpus pin verification)"
FENCE = re.compile(r"```([A-Za-z0-9_+-]*)[ \t]*\n(.*?)```", re.S)
PRE = re.compile(r"<pre\b[^>]*>(.*?)</pre>", re.S)


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


def sha_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fetch_url(url: str, dest: Path, expected: str) -> None:
    if dest.is_file() and sha_file(dest) == expected:
        return
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=120) as r:
        data = r.read()
    got = hashlib.sha256(data).hexdigest()
    if got != expected:
        raise SystemExit(f"{url}: sha256 {got} != pinned {expected}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(data)


def gh_api(path: str) -> str:
    for cmd in (["gh", "api", path], ["gh.exe", "api", path]):
        try:
            out = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
        except FileNotFoundError:
            continue
        if out.returncode == 0:
            return out.stdout
        raise SystemExit(f"gh api {path}: {out.stderr.strip()}")
    raise SystemExit("gh not found")


def arena_fetch() -> None:
    issues = []
    for page in (1, 2, 3, 4):
        raw = gh_api(f"repos/Frankcastleauditor/Solana-Audit-Arena/issues?state=all&per_page=100&page={page}")
        chunk = json.loads(raw)
        if not isinstance(chunk, list) or not chunk:
            raise SystemExit(f"arena page {page}: unexpected response")
        for it in chunk:
            issues.append({
                "number": it["number"],
                "title": it["title"],
                "labels": [{"name": l["name"]} for l in it["labels"]],
                "body": it.get("body") or "",
            })
        if page < 4:
            time.sleep(8)
    issues.sort(key=lambda i: i["number"])
    (CACHE / "arena").mkdir(parents=True, exist_ok=True)
    (CACHE / "arena" / "issues.json").write_text(
        json.dumps(issues, indent=1, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    print(f"arena: {len(issues)} issues")


def repos_fetch(units: list[dict]) -> None:
    need: dict[str, str] = {}
    for u in units:
        for f in u["files"]:
            if f["kind"] == "repo-file":
                need[f["repo"]] = f["commit"]
    for repo, commit in sorted(need.items()):
        dirname = KNOWN_REPOS.get(repo)
        if not dirname:
            raise SystemExit(f"unknown repo {repo}")
        dest = CACHE / "repos" / dirname
        if dest.is_dir():
            head = subprocess.run(["git", "-C", str(dest), "rev-parse", "HEAD"],
                                  capture_output=True, text=True)
            if head.returncode == 0 and head.stdout.strip() == commit:
                continue
            shutil.rmtree(dest)
        dest.mkdir(parents=True)
        subprocess.run(["git", "-C", str(dest), "init", "-q"], check=True)
        subprocess.run(["git", "-C", str(dest), "remote", "add", "origin",
                        f"https://github.com/{repo}.git"], check=True)
        got = subprocess.run(["git", "-C", str(dest), "fetch", "-q", "--depth", "1",
                              "origin", commit], capture_output=True, text=True)
        if got.returncode != 0:
            raise SystemExit(f"{repo}@{commit}: fetch failed: {got.stderr.strip()}")
        subprocess.run(["git", "-C", str(dest), "checkout", "-q", "FETCH_HEAD"], check=True)
        print(f"repo: {repo}@{commit[:8]}")


def pages_fetch(units: list[dict]) -> None:
    pages = {}
    for u in units:
        for f in u["files"]:
            if f["kind"] in ("page-json-block", "page-pre-block"):
                pages[f["url"]] = f["page_sha256"]
    for url, sha in pages.items():
        name = "rustfund-results.html" if "codehawks" in url else "jupiter-lend-report.html"
        fetch_url(url, CACHE / "pages" / name, sha)
        print(f"page: {name}")


def verify(units: list[dict]) -> list[str]:
    errors: list[str] = []
    arena = None
    pages: dict[str, str] = {}
    for u in units:
        for f in u["files"]:
            kind = f["kind"]
            if kind == "issue-block":
                if arena is None:
                    p = CACHE / "arena" / "issues.json"
                    if not p.is_file():
                        errors.append("missing arena/issues.json")
                        continue
                    arena = {i["number"]: i for i in json.loads(p.read_text(encoding="utf-8"))}
                it = arena.get(f["issue"])
                if it is None:
                    errors.append(f"{u['id']}: issue {f['issue']} missing")
                    continue
                if hashlib.sha256(it["body"].encode("utf-8")).hexdigest() != f["body_sha256"]:
                    errors.append(f"{u['id']}: issue body hash drift")
                    continue
                blocks = FENCE.findall(it["body"])
                if f["block"] >= len(blocks):
                    errors.append(f"{u['id']}: block index {f['block']} missing")
                    continue
                if norm_hash(blocks[f["block"]][1]) != f["block_sha256"]:
                    errors.append(f"{u['id']}: block hash mismatch")
            elif kind == "page-json-block":
                p = CACHE / "pages" / "rustfund-results.html"
                if not p.is_file():
                    errors.append("missing rustfund page")
                    continue
                text = p.read_text(encoding="utf-8", errors="replace")
                if hashlib.sha256(p.read_bytes()).hexdigest() != f["page_sha256"]:
                    errors.append(f"{u['id']}: page hash drift")
                    continue
                i = text.find("findings.getFindingOverviewsForCompetition")
                k = text.find(">", i); e = text.find("</script>", k)
                res = json.loads(json.loads(text[k + 1:e])["body"])[0]["result"]["data"]
                groups = ((res.get("highs") or []) + (res.get("mediums") or [])
                          + (res.get("lows") or []) + (res.get("gas") or []))
                grp = next((g for g in groups if g["id"] == f["group"]), None)
                issue = next((x for x in (grp or {}).get("issues", []) if x["id"] == f["issue_id"]), None)
                if issue is None:
                    errors.append(f"{u['id']}: submission not found")
                    continue
                blocks = FENCE.findall(issue.get("description") or "")
                if f["block"] >= len(blocks):
                    errors.append(f"{u['id']}: block index missing")
                    continue
                if norm_hash(blocks[f["block"]][1]) != f["block_sha256"]:
                    errors.append(f"{u['id']}: block hash mismatch")
            elif kind == "page-pre-block":
                p = CACHE / "pages" / "jupiter-lend-report.html"
                if not p.is_file():
                    errors.append("missing jupiter-lend page")
                    continue
                text = p.read_text(encoding="utf-8", errors="replace")
                if hashlib.sha256(p.read_bytes()).hexdigest() != f["page_sha256"]:
                    errors.append(f"{u['id']}: page hash drift")
                    continue
                pres = PRE.findall(text)
                if f["pre_index"] >= len(pres):
                    errors.append(f"{u['id']}: pre index missing")
                    continue
                inner = html_mod.unescape(re.sub(r"<[^>]+>", "", pres[f["pre_index"]]))
                if norm_hash(inner) != f["block_sha256"]:
                    errors.append(f"{u['id']}: pre block hash mismatch")
            elif kind == "repo-file":
                d = KNOWN_REPOS[f["repo"]]
                base = CACHE / "repos" / d
                p = base / f["path"]
                if not p.is_file():
                    errors.append(f"{u['id']}: missing {f['path']}")
                    continue
                head = subprocess.run(["git", "-C", str(base), "rev-parse", "HEAD"],
                                      capture_output=True, text=True)
                if head.returncode != 0 or head.stdout.strip() != f["commit"]:
                    errors.append(f"{u['id']}: repo not at pinned commit")
                    continue
                if norm_hash(p.read_text(encoding="utf-8", errors="replace")) != f["sha256_norm"]:
                    errors.append(f"{u['id']}: file hash mismatch")
            else:
                errors.append(f"{u['id']}: unknown pin kind {kind}")
    return errors


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args(argv[1:])
    doc = json.loads(INV.read_text(encoding="utf-8"))
    units = doc["units"]
    if not args.check:
        arena_fetch()
        pages_fetch(units)
        repos_fetch(units)
    errors = verify(units)
    if errors:
        print(f"{len(errors)} pin error(s):", file=sys.stderr)
        for e in errors[:40]:
            print("  " + e, file=sys.stderr)
        return 1
    print(f"all {len(units)} unit pins verified" + (" (offline)" if args.check else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
