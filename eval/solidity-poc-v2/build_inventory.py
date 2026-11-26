#!/usr/bin/env python3
"""Builds eval/solidity-poc-v2/inventory.json from the pinned cache.

Additive to eval/solidity-poc-v1: v1 units and split assignments are never
touched, and every v2 unit is deduped by content hash and by URL against every
opened and sealed corpus under eval/.

Units are pointers plus hashes; no third-party text is copied into the repo.
The block selector is v1's `select_sol_block`, loaded from
eval/solidity-poc-v1/build_inventory.py so the two corpora use one selector
(content-based; the fence label is only recorded).

J  Sherlock: one section of the judging repository's final report (README.md,
   "# Issue H-n" / "# Issue M-n") is one judged finding group; duplicates are
   never separate units. Only the issue body before "## Discussion" is read.
   codebase_id points at the sherlock-audit/<contest> snapshot repo.
J  Immunefi bounty write-ups (operator-supplied finder list, pointer only):
   one write-up page with a complete Foundry/Hardhat test and a payout or
   severity stated on the original page or the program's page.
X  incident reproduction test files not in any opened corpus.

Usage:
  python -I eval/solidity-poc-v2/build_inventory.py
  python -I eval/solidity-poc-v2/build_inventory.py --check
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
HERE = ROOT / "eval" / "solidity-poc-v2"
CACHE = ROOT / ".corpus" / "solidity-poc-v2"
SALT = "rg-solidity-v2-salt-bb3d119768be2a49b8417195654a5d66"

_spec = importlib.util.spec_from_file_location(
    "solidity_v1_builder", ROOT / "eval" / "solidity-poc-v1" / "build_inventory.py")
V1 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(V1)
select_sol_block = V1.select_sol_block
norm_hash = V1.norm_hash

SHER_SEC = re.compile(r"^# Issue ([HM])-(\d+): (.*?)$(.*?)(?=^# Issue [HM]-\d+:|\Z)", re.S | re.M)
SOURCE = re.compile(r"^Source:\s*(https://github\.com/sherlock-audit/\S+/issues/\d+)", re.M)

# Corpora whose units are opened or sealed; a v2 unit may not repeat any of
# their content hashes or URLs.
OPENED = [
    "eval/corpus.json",
    "eval/solana-poc/inventory.json", "eval/solana-poc/corpus.json",
    "eval/solana-poc-v2/inventory.json", "eval/solana-poc-v2/corpus.json",
    "eval/solana-poc-v3/inventory.json",
    "eval/solidity-poc-v1/inventory.json",
    "eval/near-poc-v1/inventory.json", "eval/near-poc-v1/corpus.json",
]
HEX64 = re.compile(r"^[0-9a-f]{64}$")


def split_of(uid: str) -> str:
    n = int(hashlib.sha256((SALT + "|" + uid).encode("utf-8")).hexdigest()[:8], 16) % 100
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


def tsv(path: Path) -> dict[str, str]:
    out = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if "\t" in line:
            k, v = line.split("\t", 1)
            out[k.strip()] = v.strip()
    return out


def git_blob(repo: Path, commit: str, path: str) -> bytes | None:
    """Raw blob bytes at commit:path (no eol/smudge filters), or None."""
    import subprocess
    if not re.fullmatch(r"[0-9a-f]{40}", commit or ""):
        return None
    out = subprocess.run(["git", "-C", str(repo), "cat-file", "blob", f"{commit}:{path}"],
                         capture_output=True)
    return out.stdout if out.returncode == 0 else None


def sherlock_units(cb: dict) -> tuple[list[dict], list[dict]]:
    heads = tsv(CACHE / "meta" / "sherlock-judging-heads.tsv")
    bases = tsv(CACHE / "meta" / "sherlock-codebases.tsv")
    units: list[dict] = []
    screened: list[dict] = []
    for repo in sorted((CACHE / "sherlock").iterdir()):
        name = repo.name
        # Hash and parse the git blob at the pinned commit, never the working
        # tree: core.autocrlf would make working-tree bytes host-dependent.
        blob = git_blob(repo, heads.get(name, ""), "README.md")
        if blob is None:
            screened.append({"repo": name, "sections": 0, "taken": 0, "reason": "no README.md final report"})
            continue
        # Newlines are normalised before parsing (as Python universal-newline
        # reading did), so CRLF-upstream reports parse the same on every host.
        text = blob.decode("utf-8", errors="replace").replace("\r\n", "\n").replace("\r", "\n")
        report_sha = hashlib.sha256(blob).hexdigest()
        slug = name[: -len("-judging")] if name.endswith("-judging") else name
        commit = bases.get(slug, "MISSING")
        if re.fullmatch(r"[0-9a-f]{40}", commit):
            cb[slug] = {"repo": f"https://github.com/sherlock-audit/{slug}", "commit": commit,
                        "source": "sherlock-contest-snapshot"}
        n_sec = n_take = 0
        for m in SHER_SEC.finditer(text):
            n_sec += 1
            sev, num, title, rest = m.group(1), m.group(2), m.group(3).strip(), m.group(4)
            body = rest.split("\n## Discussion", 1)[0]
            sm = SOURCE.search(body)
            sel = select_sol_block(body)
            if sel is None:
                continue
            n_take += 1
            sev_word = "High" if sev == "H" else "Medium"
            uid = f"J-sherlock-{slug}-{sev}{num}"
            units.append({
                "id": uid,
                "source": "sherlock-" + slug,
                "platform": "sherlock",
                "tier": "J",
                "kind": "test-fragment",
                "files": [{
                    "kind": "sherlock-report-section-block",
                    "repo": f"https://github.com/sherlock-audit/{name}",
                    "commit": heads.get(name),
                    "path": "README.md",
                    "report_sha256": report_sha,
                    "section": f"{sev}-{num}",
                    "issue_url": sm.group(1) if sm else None,
                    "block": sel["block"],
                    "lang": sel["lang"],
                    "fence_label": sel["fence_label"],
                    "block_sha256": norm_hash(sel["code"]),
                }],
                "outcome": "valid",
                "severity": sev_word,
                "severity_title": None,
                "severity_conflict": False,
                "severity_source": "report-section-heading",
                "outcome_source": "sherlock-final-report",
                "outcome_evidence": f'Judging repo final report heading "Issue {sev}-{num}"',
                "outcome_url": sm.group(1) if sm else f"https://github.com/sherlock-audit/{name}",
                "group_id": None,
                "group_size": 1,
                "duplicate_of": None,
                "codebase_id": slug if slug in cb else None,
                "license": "none",
                "unlicensed": True,
            })
        screened.append({"repo": name, "sections": n_sec, "taken": n_take})
    return units, screened


PRE = re.compile(r"<pre\b[^>]*>(.*?)</pre>", re.S)
JSON_BODY = re.compile(r'"body":"((?:[^"\x5c]|\x5c.)*)"')


def page_markdown(raw: str) -> str:
    """Rebuilds fenced blocks from an HTML page: every <pre> block, then any
    markdown body embedded as JSON (mirror.xyz / hashnode). Deterministic."""
    import html as html_mod
    out = []
    for m in PRE.finditer(raw):
        inner = re.sub(r"<br\s*/?>", "\n", m.group(1))
        inner = html_mod.unescape(re.sub(r"<[^>]+>", "", html_mod.unescape(inner)))
        out.append("```\n" + inner + "\n```")
    for m in JSON_BODY.finditer(raw):
        try:
            out.append(json.loads('"' + m.group(1) + '"'))
        except ValueError:
            pass
    return "\n\n".join(out)


# Bounty write-ups from the operator-supplied finder list
# (sayan011/Immunefi-bug-bounty-writeups-list @ b8277a4, pointer only). The
# outcome quote is from the ORIGINAL page named in outcome_url, never the list.
WRITEUPS = [
    {"id": "J-bounty-alchemist-dacian", "row": 32, "whitehat": "dacian", "platform": "bounty-platform-unstated",
     "pin": {"kind": "page-pre-block", "url": "https://dacian.me/28k-bounty-admin-brick-forced-revert",
             "cache": "immunefi-list/pages/row032.html"},
     "outcome": "paid", "severity": "unstated",
     "evidence": 'Page headline "$28K Bounty - Admin Brick & Forced Revert"',
     "outcome_url": "https://dacian.me/28k-bounty-admin-brick-forced-revert"},
    {"id": "J-bounty-sherlock-yield-gothicshanon", "row": 7, "whitehat": "GothicShanon89238", "platform": "sherlock-bounty",
     "pin": {"kind": "raw-file", "repo": "https://github.com/sherlock-protocol/bug-poc",
             "commit": "f5fee9404ca8ddbb2eb7e5cb9f565ae40b718673", "path": "test/Attack.sol",
             "cache": "immunefi-list/linked/sherlock-bug-poc-Attack.sol"},
     "outcome": "paid", "severity": "Critical",
     "evidence": '"Sherlock ... has paid out a $250k bounty as a reward"',
     "outcome_url": "https://mirror.xyz/0xE400820f3D60d77a3EC8018d44366ed0d334f93C/LOZF1YBcH1eBdxlC6HP223cAMeTpNgQ-Kc4EjQuxmGA"},
    {"id": "J-bounty-tranchess-floranguyen0", "row": 97, "whitehat": "floranguyen0", "platform": "immunefi",
     "pin": {"kind": "raw-file", "repo": "https://github.com/floranguyen0/tranchess-vulnerability-disclosure",
             "commit": "a270357fd8ec89f16951c1ce12657bd5465e829c", "path": "tranchessPoc.t.sol",
             "cache": "immunefi-list/linked/tranchessPoc.t.sol"},
     "outcome": "paid", "severity": "unstated",
     "evidence": '"rewarded me with the maximum bounty of $200,000"',
     "outcome_url": "https://github.com/floranguyen0/tranchess-vulnerability-disclosure"},
    {"id": "J-bounty-angle-deliriusz-vault", "row": 36, "whitehat": "deliriusz", "platform": "immunefi",
     "pin": {"kind": "raw-file", "repo": "https://github.com/deliriusz/Angle-bb-poc",
             "commit": "14f522aef28ab787ae5ea8b97a688739aec3934e", "path": "test/POC-vault.t.sol",
             "cache": "immunefi-list/linked/angle-POC-vault.t.sol"},
     "outcome": "rejected", "severity": "rejected",
     "evidence": '"Again, submitted, got rejected, asked Immunefi for help, got rejected."',
     "outcome_url": "https://medium.com/@deliriusz/stealing-in-motion-immunefi-bounty-hunting-from-different-angle-5eb03602f5c1"},
    {"id": "J-bounty-angle-deliriusz-erc777", "row": 36, "whitehat": "deliriusz", "platform": "immunefi",
     "pin": {"kind": "raw-file", "repo": "https://github.com/deliriusz/Angle-bb-poc",
             "commit": "14f522aef28ab787ae5ea8b97a688739aec3934e", "path": "test/POC-reentrancy-erc777.t.sol",
             "cache": "immunefi-list/linked/angle-POC-reentrancy-erc777.t.sol"},
     "outcome": "rejected", "severity": "rejected",
     "evidence": '"So I wrote a PoC, submitted it, and... got rejected."',
     "outcome_url": "https://medium.com/@deliriusz/stealing-in-motion-immunefi-bounty-hunting-from-different-angle-5eb03602f5c1"},
]


def writeup_units() -> list[dict]:
    units = []
    for w in WRITEUPS:
        pin = dict(w["pin"])
        path = CACHE / pin.pop("cache")
        raw = path.read_text(encoding="utf-8", errors="replace")
        if pin["kind"] == "page-pre-block":
            sel = select_sol_block(page_markdown(raw))
            if sel is None:
                raise SystemExit(f"{w['id']}: no qualifying block")
            # The page is dynamic HTML, so its bytes are not a pin; the
            # selected block's normalised hash is the verified pin.
            pin.update({"pin": "block_sha256 of the selected block (page bytes are dynamic)",
                        "block": sel["block"], "lang": sel["lang"],
                        "block_sha256": norm_hash(sel["code"])})
            kind = "test-fragment"
        else:
            if not V1.SOLTEST.search(raw):
                raise SystemExit(f"{w['id']}: file carries no Foundry test marker")
            pin["sha256_norm"] = norm_hash(raw)
            pin["url"] = f"{pin['repo'].replace('github.com', 'raw.githubusercontent.com')}/{pin['commit']}/{pin['path']}"
            kind = "test-file"
        units.append({
            "id": w["id"],
            "source": "bounty-" + w["whitehat"].lower(),
            "platform": w["platform"],
            "tier": "J",
            "kind": kind,
            "files": [pin],
            "outcome": w["outcome"],
            "severity": w["severity"],
            "severity_source": "original-writeup",
            "outcome_source": "original-writeup",
            "outcome_evidence": w["evidence"],
            "outcome_url": w["outcome_url"],
            "finder": {"list": "sayan011/Immunefi-bug-bounty-writeups-list", "commit": "b8277a4", "row": w["row"]},
            "group_id": None, "group_size": 1, "duplicate_of": None,
            "codebase_id": None,
            "license": "none", "unlicensed": True,
        })
    return units


LEARN_REPO = "https://github.com/coinspect/learn-evm-attacks"


def learn_x_units() -> list[dict]:
    base = CACHE / "x" / "learn-evm-attacks"
    commit = V1.git_head(base)
    units = []
    for p in sorted(base.glob("test/**/*.attack.sol")):
        rel = p.relative_to(base).as_posix()
        units.append({
            "id": "X-learnevm-" + re.sub(r"[^a-z0-9]+", "-", Path(rel).parent.name.lower()).strip("-")
                  + "-" + re.sub(r"[^a-z0-9]+", "-", p.name[:-len(".attack.sol")].lower()).strip("-"),
            "source": "coinspect-learn-evm-attacks",
            "platform": "incident-repro",
            "tier": "X",
            "kind": "test-file",
            "files": [{
                "kind": "repo-file",
                "repo": LEARN_REPO,
                "commit": commit,
                "path": rel,
                "sha256_norm": norm_hash(git_blob(base, commit, rel).decode("utf-8", errors="replace")),
            }],
            "real_incident": True,
            "incident": Path(rel).parent.name,
            "group_id": None, "group_size": None, "duplicate_of": None,
            "license": "MIT",
            "unlicensed": False,
            "note": "pointer + hash kept although MIT permits copying; same-incident overlap with the opened DeFiHackLabs corpus is possible (different code)",
        })
    return units


def extra_units(cb: dict) -> list[dict]:
    return writeup_units() + learn_x_units()


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args(argv[1:])
    cb: dict = {}
    sher, screened = sherlock_units(cb)
    units = sher + extra_units(cb)
    ids = [u["id"] for u in units]
    if len(ids) != len(set(ids)):
        raise SystemExit("duplicate unit ids")
    op_hash, op_url = opened_index()
    seen: dict[str, str] = {}
    for u in units:
        f = u["files"][0]
        key = f.get("block_sha256") or f.get("sha256_norm")
        u["dedupe_key"] = key
        url = (f.get("issue_url") or f.get("url") or "").rstrip("/")
        if key in op_hash:
            u["opened_duplicate"] = "content hash present in an opened/sealed corpus"
        elif url and url in op_url:
            u["opened_duplicate"] = "source URL present in an opened/sealed corpus"
        if key in seen:
            u["content_duplicate_of"] = seen[key]
        else:
            seen[key] = u["id"]
        u["split"] = split_of(u["id"])
    counted = [u for u in units if "content_duplicate_of" not in u and "opened_duplicate" not in u]
    by_tier: dict[str, int] = {}
    for u in counted:
        by_tier[u["tier"]] = by_tier.get(u["tier"], 0) + 1
    j = [u for u in counted if u["tier"] == "J" and u.get("duplicate_of") is None]
    j_by_platform: dict[str, int] = {}
    j_by_source: dict[str, int] = {}
    for u in j:
        j_by_platform[u["platform"]] = j_by_platform.get(u["platform"], 0) + 1
        j_by_source[u["source"]] = j_by_source.get(u["source"], 0) + 1
    v1 = json.loads((ROOT / "eval" / "solidity-poc-v1" / "inventory.json").read_text(encoding="utf-8"))
    v1_plat = dict(v1["counts"]["J_by_platform"])
    comb = dict(v1_plat)
    for k, v in j_by_platform.items():
        comb[k] = comb.get(k, 0) + v
    comb_total = sum(comb.values())
    split_counts = {}
    for t in sorted(by_tier):
        split_counts[t] = {s: len([u for u in counted if u["tier"] == t and u["split"] == s])
                           for s in ("DEV", "V1", "V2", "FINAL")}
    doc = {
        "schema": "realism-gate.solidity-poc-inventory/v2",
        "date": "2026-11-26",
        "salt": SALT,
        "split_rule": "int(sha256(salt + '|' + unit_id)[:8], 16) % 100: 0-39 DEV, 40-59 V1, 60-74 V2, 75-99 FINAL",
        "tiers": ["J", "X", "N", "A", "AJ"],
        "unit_rule": ("J: one Sherlock final-report section (one judged finding group) or one bounty write-up; "
                      "duplicate submissions are never separate units. X: one incident test file. "
                      "A/AJ: separate cells, never pooled (Amendment 2)."),
        "selector": "eval/solidity-poc-v1/build_inventory.py select_sol_block (content-based)",
        "codebases": dict(sorted(cb.items())),
        "units": units,
        "screened": {"sherlock": screened},
        "counts": {
            "units": len(units),
            "excluded_duplicates": len(units) - len(counted),
            "counted_by_tier": dict(sorted(by_tier.items())),
            "split_by_tier": split_counts,
            "J_by_platform": dict(sorted(j_by_platform.items())),
            "J_sources": len(j_by_source),
            "J_max_source_pct": round(100.0 * max(j_by_source.values()) / len(j), 2) if j else 0,
            "J_platform_share_v1_plus_v2_pct": {k: round(100.0 * v / comb_total, 2) for k, v in sorted(comb.items())},
            "J_v1_plus_v2_total": comb_total,
            "J_codebase_pinned": sum(1 for u in j if u.get("codebase_id")),
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
