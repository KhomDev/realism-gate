#!/usr/bin/env python3
"""Build eval/solidity-poc-v3/inventory.json from the gitignored cache.

Solidity X only: real-incident reproduction tests that are not in the opened
DeFiHackLabs pin d4bd931 and whose normalised text is not already hashed in
any opened or sealed corpus. No Code4rena Solidity J. The salt is the
scaffold salt. This script does not run realism-gate and does not label.

A raw test file has no fence, so v1's select_sol_block does not apply. The
unit is the git blob. Kind is test-file when the blob contains a Foundry
test marker. Hashes are sha256 of `git cat-file blob` bytes.

Usage:
  python -I eval/solidity-poc-v3/build_inventory.py
  python -I eval/solidity-poc-v3/build_inventory.py --check
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solidity-poc-v3"
CACHE = ROOT / ".corpus" / "solidity-poc-v3"
SALT = "rg-solidity-v3-salt-e87258db84526f58a694424d3fae3c63"
SPLIT_RULE = "int(sha256(salt + '|' + unit_id)[:8], 16) % 100: 0-39 DEV, 40-59 V1, 60-74 V2, 75-99 FINAL"

_spec = importlib.util.spec_from_file_location(
    "solidity_v1_builder", ROOT / "eval" / "solidity-poc-v1" / "build_inventory.py")
V1 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(V1)
norm_hash = V1.norm_hash

DFH_REPO = "https://github.com/SunWeb3Sec/DeFiHackLabs"
DFH_COMMIT = "8a2b37f17377c1c7584cfbe63621dbe01200cfaf"
DFH_OLD = "d4bd931e758a375ed7fcecf52322dfeab54f8c0d"
DFH_DIR = CACHE / "defihacklabs"
TWABI_REPO = "https://github.com/twabi/DeFi-Reentrancy-Hacks-Repository"
TWABI_COMMIT = "129b011dc83b89a1e659df7b9e148e390e9c0c3b"
TWABI_DIR = CACHE / "twabi"

SOLTEST = re.compile(
    r"function\s+test\w*\s*\(|vm\.(?:prank|deal|store|warp|startPrank)|assertEq\("
)
OPENED = [
    "eval/corpus.json",
    "eval/solana-poc/inventory.json", "eval/solana-poc/corpus.json",
    "eval/solana-poc-v2/inventory.json", "eval/solana-poc-v2/corpus.json",
    "eval/solana-poc-v3/inventory.json",
    "eval/solana-poc-v4/inventory.json",
    "eval/solidity-poc-v1/inventory.json",
    "eval/solidity-poc-v2/inventory.json",
    "eval/near-poc-v1/inventory.json", "eval/near-poc-v1/corpus.json",
]
HEX64 = re.compile(r"^[0-9a-f]{64}$")


def split_of(unit_id: str) -> str:
    n = int(hashlib.sha256((SALT + "|" + unit_id).encode("utf-8")).hexdigest()[:8], 16) % 100
    return "DEV" if n < 40 else "V1" if n < 60 else "V2" if n < 75 else "FINAL"


def git_blob(repo: Path, commit: str, path: str) -> bytes | None:
    out = subprocess.run(
        ["git", "-C", str(repo), "cat-file", "blob", f"{commit}:{path}"],
        capture_output=True)
    return out.stdout if out.returncode == 0 else None


def git_paths(repo: Path, commit: str) -> list[str]:
    out = subprocess.run(
        ["git", "-C", str(repo), "ls-tree", "-r", "--name-only", commit],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if out.returncode != 0:
        raise SystemExit(f"ls-tree failed in {repo}")
    return [p for p in out.stdout.splitlines() if p.endswith(".sol")]


def opened_index() -> tuple[set[str], set[str]]:
    hashes: set[str] = set()
    urls: set[str] = set()

    def walk(x, key=""):
        if isinstance(x, dict):
            for k, v in x.items():
                walk(v, k)
        elif isinstance(x, list):
            for v in x:
                walk(v, key)
        elif isinstance(x, str):
            if HEX64.match(x) and key != "salt":
                hashes.add(x)
            elif x.startswith("http") and ("url" in key.lower() or key in ("repo", "repository")):
                urls.add(x.rstrip("/"))

    for rel in OPENED:
        p = ROOT / rel
        if p.is_file():
            walk(json.loads(p.read_text(encoding="utf-8")))
    return hashes, urls


def first_comment(text: str) -> str:
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("//") and "SPDX" not in s and s not in ("//",):
            words = s.lstrip("/ ").split()
            if words:
                return " ".join(words[:15])
    return ""


def readme_quote(text: str, filename: str) -> str:
    idx = text.find(filename)
    if idx < 0:
        return ""
    window = text[max(0, idx - 900):idx]
    lost = re.findall(r"####\s+Lost:\s*(.+)", window)
    heads = re.findall(r"^###\s+(.+)$", window, re.M)
    quote = lost[-1].strip() if lost else ""
    if re.fullmatch(r"\[?(?:Amount )?Not Specified\]?", quote, re.I):
        quote = ""
    if not quote and heads:
        quote = heads[-1].strip()
    if quote:
        return " ".join(quote.split()[:15])
    return ""


def dfh_new_paths() -> list[str]:
    old = set(git_paths(DFH_DIR, DFH_OLD))
    new = []
    for path in git_paths(DFH_DIR, DFH_COMMIT):
        if path.startswith("src/test/") and path not in old:
            new.append(path)
    return sorted(new)


def dfh_norm_hashes() -> set[str]:
    out = set()
    for commit in (DFH_OLD, DFH_COMMIT):
        for path in git_paths(DFH_DIR, commit):
            raw = git_blob(DFH_DIR, commit, path)
            if raw is not None:
                out.add(norm_hash(raw.decode("utf-8", errors="replace")))
    return out


def make_unit(uid: str, cluster: str, repo: str, commit: str, path: str,
              raw: bytes, quote: str, quote_url: str, license_name: str) -> dict:
    text = raw.decode("utf-8", errors="replace")
    url = f"{repo}/blob/{commit}/{path}"
    return {
        "id": uid,
        "source": cluster,
        "platform": "incident-repro",
        "cluster": cluster,
        "tier": "X",
        "kind": "test-file",
        "files": [{
            "kind": "repo-file",
            "repo": repo,
            "commit": commit,
            "path": path,
            "url": url,
            "blob_sha256": hashlib.sha256(raw).hexdigest(),
            "sha256_norm": norm_hash(text),
        }],
        "real_incident": True,
        "incident": Path(path).stem,
        "outcome": "incident-reproduction",
        "outcome_source": "file-header" if "DeFiHackLabs" in repo else "repository-readme",
        "outcome_evidence": quote,
        "outcome_url": quote_url,
        "group_id": Path(path).stem,
        "group_size": 1,
        "duplicate_of": None,
        "license": license_name,
        "unlicensed": license_name != "Apache-2.0",
        "note": "pointer and hashes only; the test text is not copied into git",
    }


def units() -> tuple[list[dict], list[dict]]:
    op_hash, op_url = opened_index()
    dfh_hashes = dfh_norm_hashes()
    screened = []
    built = []
    for path in dfh_new_paths():
        raw = git_blob(DFH_DIR, DFH_COMMIT, path)
        text = raw.decode("utf-8", errors="replace")
        if not SOLTEST.search(text):
            screened.append({"path": path, "reason": "no Foundry test marker"})
            continue
        quote = first_comment(text)
        if not quote:
            screened.append({"path": path, "reason": "no header quote"})
            continue
        stem = re.sub(r"[^a-z0-9]+", "-", Path(path).stem.lower()).strip("-")
        url = f"{DFH_REPO}/blob/{DFH_COMMIT}/{path}"
        built.append(make_unit(
            "X-dfh-" + stem, "sunweb3sec-defihacklabs", DFH_REPO, DFH_COMMIT, path,
            raw, quote, url, "SPDX-UNLICENSED"))
    readmes = {}
    for path in git_paths(TWABI_DIR, TWABI_COMMIT):
        raw = git_blob(TWABI_DIR, TWABI_COMMIT, path)
        text = raw.decode("utf-8", errors="replace")
        h = norm_hash(text)
        if h in dfh_hashes:
            screened.append({"path": path, "reason": "same normalised text as a DeFiHackLabs file"})
            continue
        if not SOLTEST.search(text):
            screened.append({"path": path, "reason": "no Foundry test marker"})
            continue
        year = path.split("/")[1] if path.startswith("src/") else ""
        if year not in readmes:
            readmes[year] = ""
            for name in ("ReadMe.md", "ReadME.md", "README.md"):
                blob = git_blob(TWABI_DIR, TWABI_COMMIT, f"src/{year}/{name}")
                if blob:
                    readmes[year] = blob.decode("utf-8", errors="replace")
                    readmes[year + ":url"] = f"{TWABI_REPO}/blob/{TWABI_COMMIT}/src/{year}/{name}"
                    break
        quote = readme_quote(readmes.get(year, ""), Path(path).name)
        if not quote:
            screened.append({"path": path, "reason": "no readme quote for this file"})
            continue
        stem = re.sub(r"[^a-z0-9]+", "-", Path(path).stem.lower()).strip("-")
        built.append(make_unit(
            "X-twabi-" + stem, "twabi-reentrancy-hacks", TWABI_REPO, TWABI_COMMIT, path,
            raw, quote, readmes.get(year + ":url", ""), "none"))
    seen: dict[str, str] = {}
    for u in built:
        f = u["files"][0]
        key = f["sha256_norm"]
        url = f["url"].rstrip("/")
        u["dedupe_key"] = key
        u["split"] = split_of(u["id"])
        if key in op_hash or f["blob_sha256"] in op_hash:
            u["opened_duplicate"] = "content hash present in an opened or sealed corpus"
        elif url and url in op_url:
            u["opened_duplicate"] = "source URL present in an opened or sealed corpus"
        if key in seen:
            u["content_duplicate_of"] = seen[key]
        else:
            seen[key] = u["id"]
    return built, screened


def build() -> dict:
    rows, screened = units()
    counted = [u for u in rows if "opened_duplicate" not in u and "content_duplicate_of" not in u]
    by_cluster: dict[str, int] = {}
    for u in counted:
        by_cluster[u["cluster"]] = by_cluster.get(u["cluster"], 0) + 1
    return {
        "schema": "realism-gate.solidity-poc-inventory/v3",
        "date": "2026-11-27",
        "salt": SALT,
        "split_rule": SPLIT_RULE,
        "tiers": ["J", "X", "N", "A", "AJ"],
        "unit_rule": (
            "X only: one incident reproduction test file added after the opened "
            "DeFiHackLabs pin, or a different reproduction whose normalised text "
            "is not that pin. No Code4rena J. One counted unit per file."
        ),
        "selector": "raw git blob; kind test-file when a Foundry test marker is present. "
                    "v1 select_sol_block is for fenced report text and is not applied to these files.",
        "units": rows,
        "screened": screened,
        "counts": {
            "units_with_rows": len(rows),
            "counted": len(counted),
            "counted_by_tier": {"X": len(counted)},
            "counted_by_cluster": dict(sorted(by_cluster.items())),
            "split": {s: len([u for u in counted if u["split"] == s])
                      for s in ("DEV", "V1", "V2", "FINAL")},
            "screened": len(screened),
        },
        "status": "sealed at collection; not run and not labelled",
    }


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args(argv[1:])
    doc = build()
    path = HERE / "inventory.json"
    if args.check:
        have = json.loads(path.read_text(encoding="utf-8"))
        if have != doc:
            print("inventory.json does not match the cache", file=sys.stderr)
            return 1
        print(f"inventory.json ok ({doc['counts']['counted']} counted)")
        return 0
    path.write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(doc["counts"], indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
