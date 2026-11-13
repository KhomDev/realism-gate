#!/usr/bin/env python3
"""Fetch and pin the DeFiHackLabs PoC corpus, then extract the claim-binding
pairs. Network is used here only (a sparse git checkout of one pinned commit);
pairs.py-style extraction and run.py are offline.

    python eval/claims/fetch.py            # fetch the pinned corpus
    python eval/claims/fetch.py --pairs    # write pairs.yaml (offline)
    python eval/claims/fetch.py --check    # corpus commit + pairs.yaml reproducible
    python eval/claims/fetch.py --list

A pair is one PoC file plus the public incident record that makes the claim.
The claim source is the incident record in the PoC header (@KeyInfo Total Lost,
Attacker) whose @Analysis links point at the public write-ups. It is rendered
into a report file at run time; the write-up text itself is not committed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

try:
    import yaml
except ImportError:
    print("eval/claims: PyYAML is required", file=sys.stderr)
    sys.exit(2)

HERE = Path(__file__).resolve().parent
RG = HERE.parent.parent                  # realism-gate/
SOURCES = HERE / "sources.yaml"
PAIRS = HERE / "pairs.yaml"
CACHE = HERE / "cache"        # inside eval/claims, ignored by eval/claims/.gitignore
SAMPLE_N = 120


def spec() -> dict:
    return yaml.safe_load(SOURCES.read_text(encoding="utf-8"))


def corpus() -> dict:
    return spec()["corpora"][0]


def git(*args: str, cwd: Path | None = None) -> str:
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


def locate() -> Path:
    """The corpus checkout: eval/cache first, then the frozen reference clone.

    Both are gitignored; neither is committed. The frozen reference clone is
    only read.
    """
    c = corpus()
    first = (RG / c["dir"]).resolve()
    fallback = (RG / c["fallback_dir"]).resolve()
    for candidate in (first, fallback):
        if (candidate / ".git").is_dir():
            head = git("rev-parse", "HEAD", cwd=candidate)
            if head != c["commit"]:
                raise SystemExit(
                    f"{candidate}: HEAD {head[:12]} is not the pinned {c['commit'][:12]}; "
                    "delete it and re-run fetch"
                )
            return candidate
    return first


def fetch(_args) -> int:
    c = corpus()
    target = locate() if (locate() / ".git").is_dir() else (RG / c["dir"]).resolve()
    target.mkdir(parents=True, exist_ok=True)
    if not (target / ".git").is_dir():
        print(f"clone {c['repo']} at {c['commit'][:12]}")
        git("init", "-q", cwd=target)
        git("remote", "add", "origin", c["repo"], cwd=target)
        git("sparse-checkout", "set", "--no-cone", c["path"] + "/", cwd=target)
        git("fetch", "-q", "--depth", "1", "origin", c["commit"], cwd=target)
        git("checkout", "-q", "--detach", "FETCH_HEAD", cwd=target)
    head = git("rev-parse", "HEAD", cwd=target)
    if head != c["commit"]:
        raise SystemExit(f"{target}: HEAD {head[:12]} is not pinned {c['commit'][:12]}")
    files = list((target / c["path"]).rglob("*.sol"))
    print(f"{target} at {head[:12]}: {len(files)} .sol files")
    return 0


# --------------------------------------------------------------------------
# Header parsing
# --------------------------------------------------------------------------

URL = re.compile(r"https?://[^\s)'\"]+")
ADDRESS = re.compile(r"0x[0-9a-fA-F]{40}(?![0-9a-fA-F])")
TOTAL_LOST = re.compile(r"Total\s*Lost\s*:?\s*(.+)", re.I)
ATTACKER = re.compile(r"^\s*(?://|-\s*|\*\s*)?@?Attacker\??\s*:?\s*(.+)$", re.I)


def header_text(source: str) -> str:
    """The claim metadata lives in the first comment block. Take the first 80
    lines: enough for every observed style (line comments or a block comment)."""
    return "\n".join(source.splitlines()[:80])


def parse_claim(source: str) -> dict | None:
    head = header_text(source)
    lost = TOTAL_LOST.search(head)
    if not lost:
        return None
    lost_text = re.sub(r"\s+", " ", lost.group(1)).strip().strip("*/").strip()[:80]
    urls = URL.findall(head)
    analyses = [u for u in urls if any(k in u.lower() for k in ("twitter.com", "x.com", "medium.com", "blocksec", "phalcon", "neptunemutual", "solidityscan", "bitget", "substack", "mirror.xyz", "rekt", "slowmist", "certik", "peckshield", "beosin", "tenarmor"))]
    attacker_addr = None
    for line in head.splitlines():
        if re.search(r"attacker", line, re.I) and not re.search(r"attack\s*contract|attack\s*tx", line, re.I):
            found = ADDRESS.search(line)
            if found:
                attacker_addr = found.group(0)
                break
    return {
        "lost_text": lost_text,
        "attacker": attacker_addr,
        "analyses": sorted(set(analyses))[:6],
        "attack_tx": next((u for u in urls if "tx/" in u or "/tx/" in u), None),
        "urls": len(urls),
    }


def amount_asset(lost_text: str) -> tuple[str | None, str | None]:
    """Split an incident loss string into (amount, asset). Keep the raw amount
    string: it is a quote from the public incident record, not a conversion."""
    text = lost_text.replace(",", "").strip(" ~")
    m = re.match(r"(?i)^~?\s*([0-9][0-9.]*)\s*([A-Za-z$-]{2,12})?", text)
    if not m:
        return None, None
    amount = m.group(1)
    unit = (m.group(2) or "").upper()
    asset = None if unit in ("", "USD", "US$", "USD$", "K", "M") else unit
    return amount, asset


def candidates(root: Path) -> list[dict]:
    out = []
    for path in sorted((root / corpus()["path"]).rglob("*.sol")):
        if not path.name.endswith("_exp.sol"):
            continue
        source = path.read_bytes().decode("utf-8", "replace")
        claim = parse_claim(source)
        if claim is None:
            continue
        if not claim["analyses"] and not claim["attacker"]:
            continue
        rel = path.relative_to(root).as_posix()
        bid = f"defihacklabs:{rel}"
        amount, asset = amount_asset(claim["lost_text"])
        out.append({
            "id": bid,
            "poc": {
                "repo": corpus()["repo"],
                "commit": corpus()["commit"],
                "path": rel,
                "sha256": hashlib.sha256(source.encode("utf-8")).hexdigest(),
            },
            "period": path.relative_to(root / corpus()["path"]).parts[0],
            "incident": {
                "name": path.name[: -len("_exp.sol")],
                "record": "DeFiHackLabs header (@KeyInfo)",
                "lost_text": claim["lost_text"],
                "attacker_address": claim["attacker"],
                "attack_tx": claim["attack_tx"],
                "analyses": claim["analyses"],
            },
            "claim": {
                "attacker": f"address:{claim['attacker']}" if claim["attacker"] else None,
                "impact": "drain",
                "asset": asset,
                "amount": amount,
                "amount_text": claim["lost_text"],
            },
        })
    return out


def sample(items: list[dict], seed: str, n: int) -> list[dict]:
    """Deterministic draw, independent of directory or API order."""
    ranked = sorted(items, key=lambda i: hashlib.sha256(f"{seed}:{i['id']}".encode()).hexdigest())
    return sorted(ranked[:n], key=lambda i: i["id"])


def write_pairs(_args) -> int:
    root = locate()
    all_items = candidates(root)
    if not all_items:
        raise SystemExit(f"{root}: no candidate pairs; run fetch first")
    seed = spec()["seed"]
    picked = sample(all_items, seed, SAMPLE_N)
    doc = {
        "version": 1,
        "schema": "realism-gate.claims.pairs/v1",
        "note": (
            "Pointers and claim fields only. Each pair is one public PoC plus the "
            "incident record in its header that states the claim. No PoC text and "
            "no write-up text is stored here; both are fetched into eval/cache/."
        ),
        "seed": seed,
        "sampling": {
            "population": len(all_items),
            "rule": "sort by sha256(seed + ':' + id), take the first N, then sort by id",
            "n": len(picked),
        },
        "claim_source": {
            "kind": "defihacklabs-header",
            "fields": ["Total Lost", "Attacker", "Analysis links"],
            "report_render": "run.py renders each record into a report file; the linked write-ups are pointers, not vendored",
        },
        "pairs": picked,
    }
    PAIRS.write_text(yaml.safe_dump(doc, sort_keys=False, allow_unicode=True), encoding="utf-8", newline="\n")
    print(f"wrote {PAIRS.relative_to(RG).as_posix()}: {len(picked)} of {len(all_items)} candidates")
    return 0


def check(_args) -> int:
    if not PAIRS.is_file():
        print("eval/claims: no pairs.yaml; run --pairs first", file=sys.stderr)
        return 1
    doc = yaml.safe_load(PAIRS.read_text(encoding="utf-8"))
    root = locate()
    all_items = {i["id"]: i for i in candidates(root)}
    problems = []
    for pair in doc["pairs"]:
        current = all_items.get(pair["id"])
        if current is None:
            problems.append(f"pair no longer a candidate: {pair['id']}")
            continue
        if current["poc"]["sha256"] != pair["poc"]["sha256"]:
            problems.append(f"poc hash changed: {pair['id']}")
        if current["claim"] != pair["claim"]:
            problems.append(f"claim fields changed: {pair['id']}")
    again = sample(list(all_items.values()), doc["seed"], len(doc["pairs"]))
    if [p["id"] for p in again] != [p["id"] for p in doc["pairs"]]:
        problems.append("sample is not reproducible from the committed seed")
    if problems:
        for p in problems:
            print(f"MISMATCH {p}", file=sys.stderr)
        return 1
    print(f"pairs.yaml reproduces: {len(doc['pairs'])} pairs from {len(all_items)} candidates")
    return 0


def list_pins(_args) -> int:
    c = corpus()
    print(f"{c['id']}: {c['repo']}@{c['commit'][:12]} path {c['path']}")
    print(f"cache: {c['dir']}, fallback: {c['fallback_dir']}")
    return 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="fetch and pin the claim-binding PoC corpus")
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--pairs", action="store_true", help="offline: write pairs.yaml")
    group.add_argument("--check", action="store_true", help="offline: pairs.yaml still reproduces")
    group.add_argument("--list", action="store_true", help="print the pinned corpus")
    args = parser.parse_args(argv)
    if args.pairs:
        return write_pairs(args)
    if args.check:
        return check(args)
    if args.list:
        return list_pins(args)
    return fetch(args)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
