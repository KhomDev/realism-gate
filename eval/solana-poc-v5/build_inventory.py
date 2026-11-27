#!/usr/bin/env python3
"""Build eval/solana-poc-v5/inventory.json from the gitignored cache.

The salt and the split rule are the ones committed in the scaffold. This
script does not run realism-gate and does not label. Blocks inside report
text are chosen by eval/solana-poc-v3/build_inventory.py:select_block
(content, not the fence label). A raw test file is one unit; its kind is
read from the file alone, and the same selector has to accept the file
wrapped as a single fence.

Usage:
  python -I eval/solana-poc-v5/build_inventory.py
  python -I eval/solana-poc-v5/build_inventory.py --check
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
HERE = ROOT / "eval" / "solana-poc-v5"
CACHE = ROOT / ".corpus" / "solana-poc-v5"
SALT = "rg-solana-v5-salt-72b381bcd3a9c789cbdf56fa727ae773"
SPLIT_RULE = "int(sha256(salt + '|' + unit_id)[:8], 16) % 100: 0-39 DEV, 40-59 V1, 60-74 V2, 75-99 FINAL"

_spec = importlib.util.spec_from_file_location(
    "solana_v3_builder", ROOT / "eval" / "solana-poc-v3" / "build_inventory.py")
V3 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(V3)
select_block = V3.select_block
norm_hash = V3.norm_hash

METEORA_URL = "https://code4rena.com/reports/2025-08-meteora-dynamic-bonding-curve"
METEORA_HTML = CACHE / "meteora" / "report.html"
CASHIO_REPO = "https://github.com/PwnedNoMore/cashio-exploit-workshop"
CASHIO_COMMIT = "9392b12a27406a86ea89fe4b78ff97dbf73d4bbe"
CASHIO_DIR = CACHE / "cashio"
PORT_REPO = "https://github.com/otter-sec/port-sundial-pocs"
PORT_COMMIT = "289272c0be544fd82646ae1fd62ecfdaf8bc84d9"
PORT_DIR = CACHE / "port-sundial-pocs"
PDF_REPO = "https://github.com/port-finance/sundial"
PDF_COMMIT = "9395556b5742f80fb8dfd3d0d1d0cdf933547318"
PDF_PATH = "audits/port-finance-sundial-audit-public.pdf"
PDF_FILE = CACHE / "port-audit" / "port-finance-sundial-audit-public.pdf"

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
HEADING = re.compile(r"\[(F|H|M)-(\d+)\]")
PORT_UNITS = [
    ("00", "os-prt-adv-00/sundial_collateral.spec.ts",
     "OS-PRT-ADV-00 [Critical] [Resolved]: Excess Collateral Liquidations", "Critical"),
    ("01", "os-prt-adv-01/incorrect_yield.spec.ts",
     "OS-PRT-ADV-01 [High] [Resolved]: Inaccurate Yield Token Calculations", "High"),
    ("02", "os-prt-adv-02/incorrect_config_decimals.spec.ts",
     "OS-PRT-ADV-02 [High] [Resolved]: Config Changes Zero Critical Fields", "High"),
]


def split_of(unit_id: str) -> str:
    n = int(hashlib.sha256((SALT + "|" + unit_id).encode("utf-8")).hexdigest()[:8], 16) % 100
    return "DEV" if n < 40 else "V1" if n < 60 else "V2" if n < 75 else "FINAL"


def git_blob(repo: Path, commit: str, path: str) -> bytes:
    out = subprocess.run(
        ["git", "-C", str(repo), "cat-file", "blob", f"{commit}:{path}"],
        capture_output=True)
    if out.returncode != 0:
        raise SystemExit(f"missing blob {repo} {commit}:{path}")
    return out.stdout


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


def _strip_html(s: str) -> str:
    s = re.sub(r"<[^>]+>", "", s)
    return (s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
            .replace("&quot;", '"').replace("&#39;", "'"))


def meteora_sections(html: str) -> list[tuple[str, str, list[str]]]:
    """(finding id, heading text, source lines) from the rendered report."""
    events = []
    for m in re.finditer(r"<h([23])[^>]*>(.*?)</h\1>", html):
        events.append((m.start(), "h", _strip_html(m.group(2)).strip()))
    for m in re.finditer(r'class="grvsc-source">(.*?)</span></span>', html):
        events.append((m.start(), "c", _strip_html(m.group(1))))
    events.sort()
    sections: list[tuple[str, str, list[str]]] = []
    cur_id = cur_title = None
    buf: list[str] = []
    for _, kind, val in events:
        hm = HEADING.search(val) if kind == "h" else None
        if hm:
            if cur_id:
                sections.append((cur_id, cur_title, buf))
            cur_id = f"{hm.group(1)}-{hm.group(2)}"
            cur_title = val
            buf = []
        elif kind == "c" and cur_id is not None:
            buf.append(val)
    if cur_id:
        sections.append((cur_id, cur_title, buf))
    return sections


def _chunks(lines: list[str]) -> list[str]:
    blocks: list[list[str]] = []
    cur: list[str] = []
    start = re.compile(r"(pub |async |fn |describe|it|#\[|import |const |function )")
    for ln in lines:
        if (cur and ln and not ln[0].isspace() and cur[-1].strip() == "}"
                and start.match(ln)):
            blocks.append(cur)
            cur = [ln]
        else:
            cur.append(ln)
    if cur:
        blocks.append(cur)
    return ["\n".join(b) for b in blocks]


def meteora_units(page_sha: str) -> list[dict]:
    html = METEORA_HTML.read_text(encoding="utf-8", errors="replace")
    units = []
    for fid, title, lines in meteora_sections(html):
        md = "\n\n".join("```\n" + b + "\n```" for b in _chunks(lines))
        sel = select_block(md, title)
        if sel is None:
            continue
        words = re.sub(r"\[[^\]]+\]", "", title).split()
        quote = " ".join(words[:15])
        uid = f"J-c4-meteora-{fid.replace('-', '')}"
        units.append({
            "id": uid,
            "source": "code4rena-meteora-dbc",
            "platform": "code4rena",
            "cluster": "code4rena-meteora-dbc",
            "tier": "J",
            "kind": "test-fragment",
            "files": [{
                "kind": "report-page-block",
                "url": METEORA_URL,
                "page_sha256": page_sha,
                "block": sel["block"],
                "lang": sel["lang"],
                "fence_label": sel["fence_label"],
                "block_sha256": norm_hash(sel["code"]),
            }],
            "outcome": "valid",
            "severity": "Medium",
            "outcome_source": "code4rena-final-report",
            "outcome_evidence": quote,
            "outcome_url": METEORA_URL,
            "group_id": fid,
            "group_size": 1,
            "duplicate_of": None,
            "license": "none",
            "unlicensed": True,
        })
    return units


def file_unit(repo_dir: Path, commit: str, path: str, title: str) -> dict | None:
    raw = git_blob(repo_dir, commit, path)
    text = raw.decode("utf-8", errors="replace")
    sel = select_block("```\n" + text + "\n```", title)
    if sel is None:
        return None
    return {
        "blob_sha256": hashlib.sha256(raw).hexdigest(),
        "sha256_norm": norm_hash(text),
        "block_sha256": norm_hash(sel["code"]),
        "lang": sel["lang"],
        "fence_label": sel["fence_label"],
    }


def cashio_units() -> list[dict]:
    path = "poc/src/main.rs"
    raw = git_blob(CASHIO_DIR, CASHIO_COMMIT, path)
    text = raw.decode("utf-8", errors="replace")
    # v3 select_block's driver matches `fn main` and not `async fn main`.
    # This file's entry point is `async fn main`, so the whole blob is the unit.
    got = file_unit(CASHIO_DIR, CASHIO_COMMIT, path, "Cashio infinite mint")
    if got is None:
        if not re.search(r"^\s*(?:pub(?:\(crate\))?\s+)?async\s+fn\s+main\s*\(", text, re.M):
            return []
        got = {
            "blob_sha256": hashlib.sha256(raw).hexdigest(),
            "sha256_norm": norm_hash(text),
            "block_sha256": norm_hash(text),
            "lang": "rust",
            "fence_label": "",
            "selector_note": "v3 select_block does not treat async fn main as a driver; the pin is the whole blob",
        }
    url = f"{CASHIO_REPO}/blob/{CASHIO_COMMIT}/{path}"
    note = got.pop("selector_note", None)
    unit = {
        "id": "X-cashio-infinite-mint",
        "source": "pwnednomore-cashio",
        "platform": "incident-repro",
        "cluster": "cashio-2022-infinite-mint",
        "tier": "X",
        "kind": "binary-main",
        "files": [{
            "kind": "repo-file",
            "repo": CASHIO_REPO,
            "commit": CASHIO_COMMIT,
            "path": path,
            "url": url,
            **got,
        }],
        "real_incident": True,
        "incident": "Cashio infinite mint, March 2022",
        "outcome": "incident-reproduction",
        "outcome_source": "repository-readme",
        "outcome_evidence": "which allowed the attacker to print infinite amount of $CASH",
        "outcome_url": f"{CASHIO_REPO}/blob/{CASHIO_COMMIT}/README.md",
        "group_id": "cashio-2022",
        "group_size": 1,
        "duplicate_of": None,
        "license": "Apache-2.0",
        "unlicensed": False,
        "note": "pointer and hashes only; the Apache-2.0 text is not copied into git",
    }
    if note:
        unit["selector_note"] = note
    return [unit]


def port_units() -> list[dict]:
    units = []
    pdf_url = f"{PDF_REPO}/blob/{PDF_COMMIT}/{PDF_PATH}"
    for num, path, quote, sev in PORT_UNITS:
        got = file_unit(PORT_DIR, PORT_COMMIT, path, quote)
        if got is None:
            continue
        url = f"{PORT_REPO}/blob/{PORT_COMMIT}/{path}"
        units.append({
            "id": f"A-otter-port-sundial-{num}",
            "source": "ottersec-port-sundial",
            "platform": "ottersec",
            "cluster": "ottersec-port-sundial",
            "tier": "A",
            "kind": "test-file",
            "files": [{
                "kind": "repo-file",
                "repo": PORT_REPO,
                "commit": PORT_COMMIT,
                "path": path,
                "url": url,
                "sponsor_pdf": pdf_url,
                "sponsor_pdf_sha256": hashlib.sha256(PDF_FILE.read_bytes()).hexdigest(),
                **got,
            }],
            "outcome": "Resolved",
            "severity": sev,
            "outcome_source": "sponsor-published-audit-pdf",
            "outcome_evidence": quote,
            "outcome_url": pdf_url,
            "group_id": f"OS-PRT-ADV-{num}",
            "group_size": 1,
            "duplicate_of": None,
            "license": "none",
            "unlicensed": True,
        })
    return units


def annotate(units: list[dict]) -> tuple[list[dict], list[dict]]:
    op_hash, op_url = opened_index()
    seen: dict[str, str] = {}
    dropped = []
    for u in units:
        f = u["files"][0]
        key = f.get("block_sha256") or f.get("sha256_norm")
        url = (f.get("url") or "").rstrip("/")
        u["dedupe_key"] = key
        u["split"] = split_of(u["id"])
        if key in op_hash or f.get("sha256_norm") in op_hash or f.get("blob_sha256") in op_hash:
            u["opened_duplicate"] = "content hash present in an opened or sealed corpus"
        elif url and url in op_url:
            u["opened_duplicate"] = "source URL present in an opened or sealed corpus"
        if key in seen:
            u["content_duplicate_of"] = seen[key]
        else:
            seen[key] = u["id"]
        if "opened_duplicate" in u or "content_duplicate_of" in u:
            dropped.append({"id": u["id"], "reason": u.get("opened_duplicate") or u.get("content_duplicate_of")})
    return units, dropped


def build() -> dict:
    page = METEORA_HTML.read_bytes()
    units = meteora_units(hashlib.sha256(page).hexdigest())
    units += cashio_units()
    units += port_units()
    units, dropped = annotate(units)
    counted = [u for u in units if "opened_duplicate" not in u and "content_duplicate_of" not in u]
    by_tier: dict[str, int] = {}
    by_cluster: dict[str, int] = {}
    for u in counted:
        if u.get("duplicate_of"):
            continue
        by_tier[u["tier"]] = by_tier.get(u["tier"], 0) + 1
        by_cluster[u["cluster"]] = by_cluster.get(u["cluster"], 0) + 1
    return {
        "schema": "realism-gate.solana-poc-inventory/v5",
        "date": "2026-11-27",
        "salt": SALT,
        "split_rule": SPLIT_RULE,
        "tiers": ["J", "X", "N", "A", "AJ"],
        "unit_rule": (
            "J: one Code4rena Meteora final-report finding whose block "
            "select_block accepts. X: one incident reproduction file. "
            "A: one OtterSec advisory file with the sponsor PDF status. "
            "One counted unit per finding. A is not pooled with J or X."
        ),
        "selector": "eval/solana-poc-v3/build_inventory.py select_block (content-based)",
        "units": units,
        "dropped": dropped,
        "counts": {
            "units_with_rows": len(units),
            "counted": len(counted),
            "counted_by_tier": dict(sorted(by_tier.items())),
            "counted_by_cluster": dict(sorted(by_cluster.items())),
            "split": {s: len([u for u in counted if u["split"] == s])
                      for s in ("DEV", "V1", "V2", "FINAL")},
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
