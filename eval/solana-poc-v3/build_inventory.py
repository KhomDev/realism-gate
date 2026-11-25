#!/usr/bin/env python3
"""Builds eval/solana-poc-v3/inventory.json from the pinned cache.

Every unit is a pointer plus hashes; no third-party text is copied into the
repo. The cache lives in the gitignored `.corpus/solana-poc-v3/` and is
populated by `fetch.py`.

Unit shapes:
  J/X from the Solana Audit Arena: one GitHub issue body, one fenced block.
  J from CodeHawks RustFund: one submission in the archived results payload,
      one fenced block of its description.
  J from Code4rena Jupiter Lend: one <pre> code block of the public report.
  N from repository test files: one file at a pinned commit.

Kind is recorded from the artifact alone, before any realism-gate reader
exists. All sources are unlicensed, so the inventory carries pointers and
hashes only.

Usage:
  python -I eval/solana-poc-v3/build_inventory.py           # write inventory.json
  python -I eval/solana-poc-v3/build_inventory.py --check   # compare, no write
"""

from __future__ import annotations

import argparse
import hashlib
import html as html_mod
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solana-poc-v3"
CACHE = ROOT / ".corpus" / "solana-poc-v3"
SALT = "rg-solana-v3-salt-2b03962612050c100daeb8128706254e"

ARENA_REPO = "Frankcastleauditor/Solana-Audit-Arena"
ARENA_COMMIT = "81eaf902d971cc0a87c8107b23bf12c1d30525e0"
ARENA_ISSUE_URL = "https://github.com/" + ARENA_REPO + "/issues/{}"

RUSTFUND_PAGE = "https://web.archive.org/web/20260128075213id_/https://codehawks.cyfrin.io/c/2025-03-rustfund/results"
RUSTFUND_REPO = "CodeHawks-Contests/2025-03-rustfund"
RUSTFUND_COMMIT = "b5dd7b0ec01471667ae3a02520701aae405ac857"

C4_PAGE = "https://code4rena.com/reports/2026-02-jupiter-lend"

ZOLLDYK_REPO = "Zolldyk/anchor-pinochio-security-template"
ZOLLDYK_COMMIT = "3cf8950271e85b0291e943516f15f17c74510a08"

# v2 units, by Arena issue number. These are opened data and are never reused.
V2_J = {140, 142, 143, 148, 149, 150, 155, 156, 160, 165, 166, 167, 200, 233,
        234, 235, 236, 239, 243, 244, 246, 249, 255, 265, 275}
V2_X = {9, 12, 20, 25, 26, 40, 49, 56, 66, 99, 170, 186, 228, 277, 279, 289,
        303, 305, 319}
V2_ISSUES = V2_J | V2_X | {278}

# Labelled Arena issues that carry a harness block but were not used in v2.
# Selected by scanning all 349 issue bodies on 2026-11-25; see SEARCH_LOG.md.
ARENA_J = [141, 144, 147, 163, 187, 201, 203, 210, 270, 280, 281]
ARENA_X_TARGET = 60

FENCE = re.compile(r"```([A-Za-z0-9_+-]*)[ \t]*\n(.*?)```", re.S)
# A block counts as test code only when it carries a harness marker.
HARNESS = re.compile(
    r"#\[test\]|#\[tokio::test\]|#\[cfg\(test\)\]"
    r"|\bit\(\s*[\"'`]|describe\(\s*[\"'`]"
    r"|program\.methods|anchor\.workspace"
    r"|ProgramTest::|program_test|LiteSVM::|litesvm|bankrun"
)
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


def split_of(unit_id: str) -> str:
    n = int(hashlib.sha256((SALT + "|" + unit_id).encode("utf-8")).hexdigest()[:8], 16) % 100
    if n < 40:
        return "DEV"
    if n < 60:
        return "V1"
    if n < 75:
        return "V2"
    return "FINAL"


def blocks_of(body: str) -> list[tuple[int, str, str]]:
    return [(n, m.group(1) or "", m.group(2)) for n, m in enumerate(FENCE.finditer(body or ""))]


def harness_blocks(body: str) -> list[tuple[int, str, str]]:
    return [b for b in blocks_of(body) if HARNESS.search(b[2])]


def load_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def arena_units() -> list[dict]:
    issues = load_json(CACHE / "arena" / "issues.json")
    by_num = {i["number"]: i for i in issues}
    units: list[dict] = []
    for num in ARENA_J:
        it = by_num[num]
        hb = harness_blocks(it.get("body") or "")
        if not hb:
            raise SystemExit(f"arena issue {num}: no harness block")
        idx, lang, code = hb[0]
        labels = [l["name"] for l in it["labels"]]
        if "Valid" in labels:
            outcome = "valid"
        elif "duplicate" in labels:
            outcome = "duplicate"
        elif "invalid" in labels:
            outcome = "invalid"
        else:
            outcome = "unlabelled"
        sev = next((l for l in labels if l in ("Critical", "High", "Medium", "Low", "info")), None)
        if sev is None:
            m = re.search(r"\[(Critical|High|Medium|Low|info)\]", it["title"], re.I)
            sev = m.group(1).capitalize() if m else "unstated"
        body = it["body"]
        units.append({
            "id": f"J-arena-{num}",
            "source": "arena",
            "platform": "solana-audit-arena",
            "tier": "J",
            "kind": "test-fragment",
            "files": [{
                "kind": "issue-block",
                "issue": num,
                "url": ARENA_ISSUE_URL.format(num),
                "body_sha256": hashlib.sha256(body.encode("utf-8")).hexdigest(),
                "block": idx,
                "lang": lang,
                "block_sha256": norm_hash(code),
            }],
            "outcome": outcome,
            "severity": sev,
            "outcome_source": "arena-maintainer-labels",
            "outcome_evidence": "Maintainer labels: " + ", ".join(labels),
            "outcome_url": ARENA_ISSUE_URL.format(num),
            "license": "none",
            "unlicensed": True,
        })

    seen: dict[str, str] = {}
    taken = 0
    for it in sorted(issues, key=lambda i: i["number"]):
        num = it["number"]
        if it["labels"] or num in V2_ISSUES:
            continue
        hb = harness_blocks(it.get("body") or "")
        if not hb:
            continue
        idx, lang, code = hb[0]
        h = norm_hash(code)
        if h in seen:
            continue
        if taken >= ARENA_X_TARGET:
            continue
        seen[h] = f"X-arena-{num}"
        taken += 1
        units.append({
            "id": f"X-arena-{num}",
            "source": "arena",
            "platform": "solana-audit-arena",
            "tier": "X",
            "kind": "test-fragment",
            "files": [{
                "kind": "issue-block",
                "issue": num,
                "url": ARENA_ISSUE_URL.format(num),
                "body_sha256": hashlib.sha256((it["body"] or "").encode("utf-8")).hexdigest(),
                "block": idx,
                "lang": lang,
                "block_sha256": h,
            }],
            "license": "none",
            "unlicensed": True,
        })
    return units


def rustfund_units() -> list[dict]:
    page = (CACHE / "pages" / "rustfund-results.html").read_bytes()
    text = page.decode("utf-8", "replace")
    i = text.find("findings.getFindingOverviewsForCompetition")
    k = text.find(">", i)
    e = text.find("</script>", k)
    res = json.loads(json.loads(text[k + 1:e])["body"])[0]["result"]["data"]
    groups = ((res.get("highs") or []) + (res.get("mediums") or [])
              + (res.get("lows") or []) + (res.get("gas") or []))
    page_sha = hashlib.sha256(page).hexdigest()
    units: list[dict] = []
    seen_issue: set[str] = set()
    for g in groups:
        sev = g["severity"]
        for it in g.get("issues", []):
            if it["id"] in seen_issue:
                # Seven submissions are listed under two report groups; keep the
                # first (highest-severity) occurrence only.
                continue
            hb = harness_blocks(it.get("description") or "")
            if not hb:
                continue
            seen_issue.add(it["id"])
            idx, lang, code = hb[0]
            dup = it["id"] != g["selectedIssueId"]
            units.append({
                "id": f"J-ch-rustfund-{it['id']}",
                "source": "codehawks-rustfund",
                "platform": "codehawks",
                "tier": "J",
                "kind": "test-fragment",
                "files": [{
                    "kind": "page-json-block",
                    "url": RUSTFUND_PAGE,
                    "page_sha256": page_sha,
                    "group": g["id"],
                    "issue_id": it["id"],
                    "block": idx,
                    "lang": lang,
                    "block_sha256": norm_hash(code),
                }],
                "outcome": "valid",
                "severity": sev.capitalize(),
                "duplicate_in_group": dup,
                "selected_issue": g["selectedIssueId"],
                "outcome_source": "codehawks-results",
                "outcome_evidence": f'Results page heading "{sev.capitalize()} Risk Findings"; submission in the judged report',
                "outcome_url": RUSTFUND_PAGE,
                "license": "none",
                "unlicensed": True,
            })
    return units


def c4_units() -> list[dict]:
    page = (CACHE / "pages" / "jupiter-lend-report.html").read_bytes()
    text = page.decode("utf-8", "replace")
    want = None
    for n, m in enumerate(PRE.finditer(text)):
        inner = re.sub(r"<[^>]+>", "", m.group(1))
        inner = html_mod.unescape(inner)
        if "#[cfg(test)]" in inner and "#[test]" in inner:
            want = (n, inner)
            break
    if want is None:
        raise SystemExit("jupiter-lend: no #[cfg(test)] block found")
    n, inner = want
    return [{
        "id": "J-c4-jupiter-lend-L07",
        "source": "code4rena-jupiter-lend",
        "platform": "code4rena",
        "tier": "J",
        "kind": "rust-unit-test",
        "files": [{
            "kind": "page-pre-block",
            "url": C4_PAGE,
            "page_sha256": hashlib.sha256(page).hexdigest(),
            "finding": "L-07",
            "pre_index": n,
            "block_sha256": norm_hash(inner),
        }],
        "outcome": "valid",
        "severity": "Low",
        "outcome_source": "code4rena-report",
        "outcome_evidence": 'Report lists "L-07" under "Low Risk" with an executable Rust test',
        "outcome_url": C4_PAGE,
        "license": "none",
        "unlicensed": True,
    }]


N_FILES = [
    ("arena", "Frankcastleauditor-Solana-Audit-Arena", ARENA_REPO, ARENA_COMMIT, [
        "stake-flow/tests/stake-flow.ts",
        "x402-Meridian/tests/permit2.ts",
        "x402-Meridian/tests/x402.ts",
        "x402-Meridian/tests/x402_permit2.ts",
        "Zenon/tests/complete-curve.ts",
        "Zenon/tests/trade-tokens.ts",
        "FrankSol/programs/stake_v2/tests/test_initialize.rs",
        "FrankSol/programs/yield_generator/tests/test_yield_generator.rs",
        "Solana_Autonomix/relay402/tests/1-program.ts",
        "Solana_Autonomix/relay402/tests/2-e2e.ts",
    ]),
    ("zolldyk", "Zolldyk-anchor-pinochio-security-template", ZOLLDYK_REPO, ZOLLDYK_COMMIT, [
        "patterns/01-missing-validation/tests/pinocchio-comparison.ts",
        "patterns/02-authority-checks/tests/pinocchio-comparison.ts",
        "patterns/03-unsafe-arithmetic/tests/pinocchio-comparison.ts",
        "patterns/03-unsafe-arithmetic/tests/edge-cases.ts",
        "patterns/03-unsafe-arithmetic/tests/token-manipulation.ts",
    ]),
    ("rustfund", "CodeHawks-Contests-2025-03-rustfund", RUSTFUND_REPO, RUSTFUND_COMMIT, [
        "tests/rustfund.ts",
    ]),
]


def n_units() -> list[dict]:
    units: list[dict] = []
    for src, dirname, repo, commit, paths in N_FILES:
        for p in paths:
            f = CACHE / "repos" / dirname / p
            if not f.is_file():
                raise SystemExit(f"missing N file {f}")
            data = f.read_text(encoding="utf-8", errors="replace")
            uid = "N-" + src + "-" + re.sub(r"[^a-z0-9]+", "-", p.lower()).strip("-")
            unit = {
                "id": uid,
                "source": src,
                "platform": "solana-audit-arena" if src == "arena" else (
                    "codehawks" if src == "rustfund" else "teaching-repo"),
                "tier": "N",
                "kind": "test-file",
                "files": [{
                    "kind": "repo-file",
                    "repo": repo,
                    "commit": commit,
                    "path": p,
                    "sha256_norm": norm_hash(data),
                }],
                "license": "none",
                "unlicensed": True,
            }
            if src in ("arena", "zolldyk"):
                unit["reused_source"] = "same repository as v2 units; this file is not one of them"
            units.append(unit)
    return units


EXCLUDED = [
    {"source": "codehawks-ssswap", "count": 0,
     "reason": "archived results parsed: 10 valid finding groups, but no submission body contains a test-harness code block (Rust snippets only)"},
    {"source": "jito-restaking", "count": 5,
     "reason": "all five findings are used or screened in v2; no fresh unit"},
    {"source": "code4rena-2025-08-solana-foundation", "count": 8,
     "reason": "public report has no High/Medium and no test code in any Low/QA item"},
    {"source": "code4rena-2025-01-pump-science", "count": 3,
     "reason": "H-01/M-01/M-02 and QA are P1 data; report code blocks are source quotes"},
    {"source": "code4rena-2026-02-jupiter-lend", "count": 14,
     "reason": "the other 14 report items contain source/fix snippets, not test code"},
    {"source": "immunefi-bugfix-raydium", "count": 2,
     "reason": "both Raydium bug-fix reviews publish snippets and a narrative; no runnable test"},
    {"source": "cantina-tensor", "count": 234,
     "reason": "234 findings submitted, but Cantina publishes no individual finding or PoC to anonymous readers"},
    {"source": "hats-finance", "count": 0,
     "reason": "no Solana competition repository in the hats-finance org; Dappslap has no public findings repo"},
    {"source": "sherlock", "count": 461,
     "reason": "only Orderly and WOOFi are Solana; both fully screened in P1 (7+1 coded units, no more)"},
    {"source": "code4rena-others", "count": 878,
     "reason": "org listing (881 repos) has three Solana repos besides Anchor 2022; all handled above"},
    {"source": "arena", "count": 15,
     "reason": "labelled issues with no harness block in the body"},
    {"source": "arena", "count": 164,
     "reason": "unlabelled issues with no harness block, or beyond the 60-unit X target after dedupe"},
    {"source": "arena", "count": 25,
     "reason": "labelled Arena issues already used as v2 J units (opened data)"},
    {"source": "arena", "count": 19,
     "reason": "unlabelled Arena issues already used as v2 X units (opened data)"},
    {"source": "solodit", "count": 0,
     "reason": "Vercel security checkpoint (HTTP 429) on every request; not readable without a browser"},
    {"source": "codehawks-live", "count": 0,
     "reason": "codehawks.cyfrin.io live pages return a Vercel security checkpoint; the archived results payload was used instead"},
    {"source": "codehawks-rustfund", "count": 326,
     "reason": "all 326 judged submissions parsed; 54 carry a harness block, 47 unique by content"},
    {"source": "codehawks-rustfund", "count": 7,
     "reason": "seven submissions are listed under two report groups; the first (highest-severity) occurrence is kept and the second skipped"},
]


def dedupe(units: list[dict]) -> None:
    seen: dict[str, str] = {}
    for u in units:
        key = u["files"][0].get("block_sha256") or u["files"][0].get("sha256_norm")
        u["dedupe_key"] = key
        if key in seen:
            u["duplicate_of"] = seen[key]
        else:
            seen[key] = u["id"]


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--check", action="store_true")
    args = p.parse_args(argv[1:])
    units = arena_units() + rustfund_units() + c4_units() + n_units()
    ids = [u["id"] for u in units]
    if len(ids) != len(set(ids)):
        raise SystemExit("duplicate unit ids")
    dedupe(units)
    for u in units:
        u["split"] = split_of(u["id"])
    counted = [u for u in units if "duplicate_of" not in u]
    by_tier: dict[str, int] = {}
    for u in counted:
        by_tier[u["tier"]] = by_tier.get(u["tier"], 0) + 1
    by_source: dict[str, int] = {}
    for u in counted:
        by_source[u["source"]] = by_source.get(u["source"], 0) + 1
    by_platform: dict[str, int] = {}
    for u in counted:
        by_platform[u["platform"]] = by_platform.get(u["platform"], 0) + 1
    j = [u for u in counted if u["tier"] == "J"]
    j_by_source: dict[str, int] = {}
    j_by_platform: dict[str, int] = {}
    for u in j:
        j_by_source[u["source"]] = j_by_source.get(u["source"], 0) + 1
        j_by_platform[u["platform"]] = j_by_platform.get(u["platform"], 0) + 1
    doc = {
        "schema": "realism-gate.solana-poc-inventory/v3",
        "date": "2026-11-25",
        "salt": SALT,
        "collected": "2026-11-25",
        "split_rule": "int(sha256(salt + '|' + unit_id)[:8], 16) % 100: 0-39 DEV, 40-59 V1, 60-74 V2, 75-99 FINAL",
        "kinds": {
            "test-file": "a test file (Rust #[test]/#[tokio::test] or TS it/test) meant to drive a harness",
            "test-fragment": "a fenced block (issue or report) meant to be added to an existing test",
            "rust-unit-test": "a Rust #[test] that calls program functions directly, driving no Solana harness",
            "script": "a TS/JS script run with node (no it/test)",
            "binary-main": "a Rust binary with fn main",
            "ctf-solve-program": "an on-chain attacker program (entrypoint)",
        },
        "source_notes": {
            "arena": "Solana Audit Arena, GitHub issues. v2 used issues 9,12,20,25,26,40,49,56,66,99,140,142,143,148,149,150,155,156,160,165,166,167,170,186,200,228,233-236,239,243,244,246,249,255,265,275,277,279,289,303,305,319 and dropped 278; none is reused.",
            "codehawks-rustfund": "CodeHawks First Flight #36 results, archived snapshot. Every issue in the published report groups is a judged submission; issues other than selectedIssueId are published duplicates.",
            "code4rena-jupiter-lend": "Code4rena public report 2026-02. L-07 is the only finding with an executable test.",
            "zolldyk": "secure-path comparison and edge-case tests; the exploit-demo.ts files are v2 X units and are not reused.",
            "rustfund": "the contest program's own test suite (secure path).",
        },
        "units": units,
        "excluded": EXCLUDED,
        "counts": {
            "units": len(units),
            "duplicates": len(units) - len(counted),
            "counted_by_tier": dict(sorted(by_tier.items())),
            "counted_by_source": dict(sorted(by_source.items())),
            "counted_by_platform": dict(sorted(by_platform.items())),
            "J_by_source": dict(sorted(j_by_source.items())),
            "J_by_platform": dict(sorted(j_by_platform.items())),
            "J_source_share_pct": {k: round(100.0 * v / len(j), 2) for k, v in sorted(j_by_source.items())},
            "J_platform_share_pct": {k: round(100.0 * v / len(j), 2) for k, v in sorted(j_by_platform.items())},
            "split": {s: len([u for u in counted if u["split"] == s]) for s in ("DEV", "V1", "V2", "FINAL")},
        },
        "cap_note": ("40% is enforced on `source` at collection. Two views are reported because the "
                     "fresh data is concentrated. J origins: arena 11, codehawks-rustfund 47, "
                     "code4rena-jupiter-lend 1; J platforms: solana-audit-arena 11, codehawks 47, "
                     "code4rena 1. A strict 40%-per-J-origin cap admits at most 5 units "
                     "(2+2+1); see SEARCH_LOG.md for the FINAL arithmetic."),
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
