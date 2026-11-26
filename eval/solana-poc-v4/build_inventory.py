#!/usr/bin/env python3
"""Builds eval/solana-poc-v4/inventory.json from the pinned cache.

Additive to eval/solana-poc-v3: v3 units and split assignments are never
touched, and every v4 unit is deduped by content hash and by URL against every
opened and sealed corpus under eval/.

Every unit is a pointer plus hashes; no third-party text is copied into the
repo. The cache lives in the gitignored `.corpus/solana-poc-v4/` and is
populated by `fetch.py`. The block selector is v3's `select_block`, loaded from
eval/solana-poc-v3/build_inventory.py (content-based; the fence label is only
recorded).

Unit shapes:
  J  Code4rena Lavarage Invitational: one findings-repo issue body, one block.
     Grouped by judged finding (duplicate-N label -> group N); one counted
     unit per group, the others carry duplicate_of.
  A  Cantina private review (Solana Foundation multi-delegator, Mar 2026):
     one finding's "Proof of Concept" span of the original report PDF, as
     extracted by `pdftotext -layout` (xpdf 4.06). Separate cell (Amendment 2).

Usage:
  python -I eval/solana-poc-v4/build_inventory.py           # write inventory.json
  python -I eval/solana-poc-v4/build_inventory.py --check   # compare, no write
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solana-poc-v4"
CACHE = ROOT / ".corpus" / "solana-poc-v4"
SALT = "rg-solana-v4-salt-d43343efde75d096d1005d85de7aef82"

_spec = importlib.util.spec_from_file_location(
    "solana_v3_builder", ROOT / "eval" / "solana-poc-v3" / "build_inventory.py")
V3 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(V3)
select_block = V3.select_block
norm_hash = V3.norm_hash

LAV_REPO = "code-423n4/2024-04-lavarage-findings"
LAV_ISSUE_URL = "https://github.com/" + LAV_REPO + "/issues/{}"
LAV_REPORT = "https://code4rena.com/reports/2024-04-lavarage"

CANTINA_SF_URL = "https://cdn.cantina.xyz/reports/cantina_solana_foundation_mar2026.pdf"
CANTINA_SF_PDF = CACHE / "cantina-pdf" / "cantina_solana_foundation_mar2026.pdf"
CANTINA_SF_TXT = CACHE / "cantina-pdf" / "cantina_solana_foundation_mar2026.txt"
CANTINA_SEV = {"3.1": "High", "3.2": "Medium", "3.3": "Low", "3.4": "Informational"}

OPENED = [
    "eval/corpus.json",
    "eval/solana-poc/inventory.json", "eval/solana-poc/corpus.json",
    "eval/solana-poc-v2/inventory.json", "eval/solana-poc-v2/corpus.json",
    "eval/solana-poc-v3/inventory.json",
    "eval/solidity-poc-v1/inventory.json",
    "eval/near-poc-v1/inventory.json", "eval/near-poc-v1/corpus.json",
]
HEX64 = re.compile(r"^[0-9a-f]{64}$")
DROPPED: list[dict] = []


def split_of(unit_id: str) -> str:
    n = int(hashlib.sha256((SALT + "|" + unit_id).encode("utf-8")).hexdigest()[:8], 16) % 100
    return "DEV" if n < 40 else "V1" if n < 60 else "V2" if n < 75 else "FINAL"


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


def lavarage_units() -> list[dict]:
    issues = json.loads((CACHE / "c4" / "lavarage-issues.json").read_text(encoding="utf-8"))
    units: list[dict] = []
    for it in sorted(issues, key=lambda i: i["number"]):
        num = it["number"]
        sel = select_block(it.get("body") or "", it.get("title") or "")
        if sel is None:
            if num != 1:
                DROPPED.append({"id": f"J-c4-lavarage-{num}", "reason": "no qualifying block"})
            continue
        labels = [l["name"] for l in it["labels"]]
        dup = next((int(l.split("-", 1)[1]) for l in labels if re.fullmatch(r"duplicate-\d+", l)), None)
        if "3 (High Risk)" in labels:
            sev = "High"
        elif "2 (Med Risk)" in labels:
            sev = "Medium"
        elif "QA (Quality Assurance)" in labels:
            sev = "QA"
        else:
            sev = "unstated"
        if "unsatisfactory" in labels or "invalid" in labels:
            outcome = "invalid"
        elif dup is not None:
            outcome = "duplicate"
        else:
            outcome = "valid"
        body = it["body"]
        units.append({
            "id": f"J-c4-lavarage-{num}",
            "source": "code4rena-lavarage",
            "platform": "code4rena",
            "tier": "J",
            "kind": "test-fragment",
            "files": [{
                "kind": "issue-block",
                "repo": LAV_REPO,
                "issue": num,
                "url": LAV_ISSUE_URL.format(num),
                "body_sha256": hashlib.sha256(body.encode("utf-8")).hexdigest(),
                "block": sel["block"],
                "lang": sel["lang"],
                "fence_label": sel["fence_label"],
                "block_sha256": norm_hash(sel["code"]),
            }],
            "outcome": outcome,
            "severity": sev,
            "severity_title": None,
            "severity_conflict": False,
            "severity_source": "issue-label",
            "outcome_source": "code4rena-judge-labels",
            "outcome_evidence": "Judge labels: " + ", ".join(l for l in labels if not l.startswith(":robot:")),
            "outcome_url": LAV_ISSUE_URL.format(num),
            "group_key": dup if dup is not None else num,
            "model_target": {"repo": "https://github.com/code-423n4/2024-04-lavarage",
                             "commit": None,
                             "note": "sponsor snapshot repository returns 'Repository not found' on 2026-11-26"},
            "license": "none",
            "unlicensed": True,
        })
    return units


def cantina_sections(text: str) -> list[tuple[str, str, str]]:
    """(section number, title, body) for each 3.x.y finding after the TOC."""
    start = text.find("\n3.1 High Risk\n")
    body = text[start:] if start >= 0 else text
    heads = list(re.finditer(r"^(3\.\d\.\d+) (.+)$", body, re.M))
    out = []
    for i, m in enumerate(heads):
        end = heads[i + 1].start() if i + 1 < len(heads) else len(body)
        out.append((m.group(1), m.group(2).strip(), body[m.start():end]))
    return out


def cantina_units() -> list[dict]:
    pdf_sha = hashlib.sha256(CANTINA_SF_PDF.read_bytes()).hexdigest()
    text = CANTINA_SF_TXT.read_text(encoding="utf-8", errors="replace").replace("\x0c", "\n")
    units: list[dict] = []
    for num, title, sec in cantina_sections(text):
        i = sec.find("Proof of Concept:")
        if i < 0:
            continue
        j = sec.find("Recommendation:", i)
        span = sec[i:j if j > 0 else len(sec)]
        # A PDF has no fences: a test log printed after the test is its own
        # block in the original, so the span is cut where the log begins.
        log = V3.LOG_LINE.search(span)
        if log:
            span = span[:log.start()]
        sel = select_block("```\n" + span + "\n```", title)
        if sel is None:
            DROPPED.append({"id": f"A-cantina-sf-multidelegator-{num}", "reason": "PoC span has no test-driver marker"})
            continue
        status = re.search(r"^Solana Foundation: (.+)$", sec, re.M)
        verified = re.search(r"^Cantina Managed: (.+)$", sec, re.M)
        sev = CANTINA_SEV[num.rsplit(".", 1)[0]]
        units.append({
            "id": f"A-cantina-sf-multidelegator-{num}",
            "source": "cantina-solana-foundation-multidelegator",
            "platform": "cantina-private-review",
            "tier": "A",
            "kind": "test-fragment",
            "files": [{
                "kind": "pdf-poc-span",
                "url": CANTINA_SF_URL,
                "pdf_sha256": pdf_sha,
                "extractor": "pdftotext -layout (xpdf 4.06)",
                "section": num,
                "lang": sel["lang"],
                "block_sha256": norm_hash(sel["code"]),
            }],
            "outcome": "valid",
            "severity": sev,
            "severity_source": "report-section-heading",
            "sponsor_status": status.group(1).strip() if status else None,
            "outcome_source": "cantina-report",
            "outcome_evidence": (f'Report section {num} under "{sev}"; sponsor: '
                                 f'{status.group(1).strip()[:40] if status else "unstated"}; '
                                 f'Cantina: {verified.group(1).strip()[:30] if verified else "unstated"}'),
            "outcome_url": CANTINA_SF_URL,
            "model_target": {"note": "report names commit 18a50bc2 of multi-delegator; repository not located"},
            "license": "none",
            "unlicensed": True,
        })
    return units


EXCLUDED = [
    {"source": "code4rena-lavarage", "count": 28,
     "reason": "findings-repo issues with no qualifying test block (34 issues, 6 with a block); issue 1 is the disclosures stub"},
    {"source": "sherlock-zetachain-cross-chain-2025", "count": 0,
     "reason": "judging repo private (404); the final report (mirror copy, finder only) has 12 Solana findings and none carries Solana test code; the one test block (M-1) is Sui Move"},
    {"source": "warden-portfolio-anticsdecoded-jupiter-lend", "count": 0,
     "reason": "the public Code4rena Jupiter Lend report does not credit the warden or list the flashloan residual-debt item"},
    {"source": "cantina-solana-private-reviews", "count": 23,
     "reason": "23 of 24 Solana-touching Cantina reports (436 findings) carry no test-driver code in the original PDF"},
]


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--check", action="store_true")
    args = p.parse_args(argv[1:])
    units = lavarage_units() + cantina_units()
    ids = [u["id"] for u in units]
    if len(ids) != len(set(ids)):
        raise SystemExit("duplicate unit ids")
    op_hash, op_url = opened_index()
    seen: dict[str, str] = {}
    for u in units:
        f = u["files"][0]
        key = f["block_sha256"]
        u["dedupe_key"] = key
        if key in op_hash:
            u["opened_duplicate"] = "content hash present in an opened/sealed corpus"
        elif f.get("url", "").rstrip("/") in op_url and f["kind"] != "pdf-poc-span":
            u["opened_duplicate"] = "source URL present in an opened/sealed corpus"
        if key in seen:
            u["content_duplicate_of"] = seen[key]
        else:
            seen[key] = u["id"]
    # Grouping by judged finding (the RustFund rule).
    groups: dict[str, list[dict]] = {}
    for u in units:
        if u["source"] == "code4rena-lavarage":
            groups.setdefault(f"lavarage-{u.pop('group_key')}", []).append(u)
    for gid, members in groups.items():
        primary = f"J-c4-lavarage-{gid.split('-')[1]}"
        ids_ = [m["id"] for m in members]
        rep = primary if primary in ids_ else sorted(ids_)[0]
        for u in members:
            u["group_id"] = gid
            u["group_size"] = len(members)
            u["duplicate_of"] = None if u["id"] == rep else rep
    for u in units:
        if "group_id" not in u:
            u["group_id"] = None
            u["group_size"] = 1 if u["tier"] in ("J", "A") else None
            u["duplicate_of"] = None
        u["split"] = split_of(u["id"])
    counted = [u for u in units if "content_duplicate_of" not in u and "opened_duplicate" not in u]
    distinct = [u for u in counted if u.get("duplicate_of") is None]
    by_tier: dict[str, int] = {}
    for u in distinct:
        by_tier[u["tier"]] = by_tier.get(u["tier"], 0) + 1
    split_counts = {t: {s: len([u for u in distinct if u["tier"] == t and u["split"] == s])
                        for s in ("DEV", "V1", "V2", "FINAL")} for t in sorted(by_tier)}
    by_source: dict[str, int] = {}
    for u in distinct:
        by_source[u["source"]] = by_source.get(u["source"], 0) + 1
    excluded = list(EXCLUDED)
    if DROPPED:
        excluded.append({"source": "content-classification", "count": len(DROPPED),
                         "reason": "candidate had no qualifying block after content-based classification",
                         "ids": sorted(d["id"] for d in DROPPED)})
    doc = {
        "schema": "realism-gate.solana-poc-inventory/v4",
        "date": "2026-11-26",
        "salt": SALT,
        "collected": "2026-11-26",
        "split_rule": "int(sha256(salt + '|' + unit_id)[:8], 16) % 100: 0-39 DEV, 40-59 V1, 60-74 V2, 75-99 FINAL",
        "tiers": ["J", "X", "N", "A", "AJ"],
        "selector": "eval/solana-poc-v3/build_inventory.py select_block (content-based)",
        "kinds": dict(V3.__dict__.get("KINDS", {})) or {
            "test-fragment": "a block (issue, report or PDF PoC span) meant to be added to an existing test",
        },
        "units": units,
        "excluded": excluded,
        "counts": {
            "units": len(units),
            "submissions_counted": len(counted),
            "distinct_findings_by_tier": dict(sorted(by_tier.items())),
            "distinct_by_source": dict(sorted(by_source.items())),
            "split_by_tier_distinct": split_counts,
        },
    }
    text = json.dumps(doc, indent=2, ensure_ascii=False) + "\n"
    out = HERE / "inventory.json"
    if args.check:
        if out.read_text(encoding="utf-8") != text:
            print("inventory.json does not match the builder and the cache", file=sys.stderr)
            return 1
        print("inventory.json ok")
        return 0
    out.write_text(text, encoding="utf-8", newline="\n")
    print(json.dumps(doc["counts"], indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
