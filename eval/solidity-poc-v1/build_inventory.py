#!/usr/bin/env python3
"""Builds eval/solidity-poc-v1/inventory.json from the pinned cache.

Units are pointers plus hashes; no third-party text is copied into the repo.

J  Code4rena: one report section (one judged finding group) is one unit. The
   report lists only unique findings; duplicate submissions are never separate
   units. The PoC is the first code block in the section that carries a Foundry
   test marker. codebase_id points at the contest's sponsor snapshot repo.
X  incident / teaching exploit test files (DeFiHackLabs new files, Immunefi
   bug-fix PoC repo, two CTF solve repositories), no judgement.
N  secure-path test files from two audited contest codebases.

Usage:
  python -I eval/solidity-poc-v1/build_inventory.py
  python -I eval/solidity-poc-v1/build_inventory.py --check
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solidity-poc-v1"
CACHE = ROOT / ".corpus" / "solidity-poc-v1"
SALT = "rg-solidity-v1-salt-e3c5336cedf1f185f432914f5f2ab1ec"

SEC = re.compile(r'^## \[\[([HM])-(\d+)\]\s*(.*?)\]\((https://github\.com/[^)]+)\)(.*?)(?=^## \[\[|\n# |\Z)', re.S | re.M)
POCTEST = re.compile(r'function\s+test\w*\s*\(|function\s+invariant_\w*\s*\(|contract\s+\w*Test\w*\s*(?:is|\{)|vm\.(?:prank|deal|store|warp|startPrank)|assertEq\(|assertTrue\(|assertLt\(|assertGt\(')
FENCE = re.compile(r'```(?:solidity)?\n(.*?)```', re.S)


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


def split_of(uid: str) -> str:
    n = int(hashlib.sha256((SALT + "|" + uid).encode()).hexdigest()[:8], 16) % 100
    return "DEV" if n < 40 else "V1" if n < 60 else "V2" if n < 75 else "FINAL"


def git_head(repo: Path) -> str:
    import subprocess
    out = subprocess.run(["git", "-C", str(repo), "rev-parse", "HEAD"],
                         capture_output=True, text=True, encoding="utf-8", errors="replace")
    if out.returncode != 0:
        raise SystemExit(f"no HEAD for {repo}")
    return out.stdout.strip()


def codebases() -> dict:
    out = {}
    for line in (CACHE / "c4-codebases.tsv").read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        slug, commit = line.split("\\t", 1)
        ok = bool(re.fullmatch(r"[0-9a-f]{40}", commit))
        out[slug] = {
            "repo": f"https://github.com/code-423n4/{slug}",
            "commit": commit if ok else None,
            "source": "c4-sponsor-snapshot",
        }
    return out


def contest_jsons(repo: Path) -> int:
    d = repo / "data"
    return len(list(d.glob("*.json"))) if d.is_dir() else 0


def c4_units(cb: dict) -> list[dict]:
    units = []
    for repo in sorted(CACHE.glob("repos/c4-*")):
        if repo.name.startswith("c4-code-"):
            continue
        rp = repo / "report.md"
        if not rp.is_file():
            continue
        slug = repo.name[3:]
        text = rp.read_text(encoding="utf-8", errors="replace")
        report_sha = sha_file(rp)
        fcommit = git_head(repo)
        for m in SEC.finditer(text):
            sev, num, title, issue_url, body = m.group(1), m.group(2), m.group(3), m.group(4), m.group(5)
            blocks = FENCE.findall(body)
            idx = next((i for i, c in enumerate(blocks) if POCTEST.search(c)), None)
            if idx is None:
                continue
            uid = f"J-c4-{slug}-{sev}{num}"
            units.append({
                "id": uid,
                "source": slug,
                "platform": "code4rena",
                "tier": "J",
                "kind": "test-fragment",
                "files": [{
                    "kind": "report-section-block",
                    "repo": f"https://github.com/code-423n4/{slug}-findings",
                    "commit": fcommit,
                    "path": "report.md",
                    "report_sha256": report_sha,
                    "section": f"{sev}-{num}",
                    "issue_url": issue_url,
                    "block": idx,
                    "block_sha256": norm_hash(blocks[idx]),
                }],
                "outcome": "valid",
                "severity": "High" if sev == "H" else "Medium",
                "severity_source": "report-section-heading",
                "outcome_source": "code4rena-report",
                "outcome_evidence": f'Report heading "{("High" if sev == "H" else "Medium")} Risk Findings"; section [{sev}-{num}]',
                "outcome_url": issue_url,
                "group_id": None,
                "group_size": 1,
                "duplicate_of": None,
                "contest_submissions": contest_jsons(repo),
                "codebase_id": slug if cb.get(slug, {}).get("commit") else None,
                "license": "none",
                "unlicensed": True,
            })
    return units


def repo_file_unit(uid: str, tier: str, repo_name: str, repo_url: str, commit: str,
                   path: str, kind: str, real_incident, note: str | None = None) -> dict:
    p = CACHE / "repos" / repo_name / path
    if not p.is_file():
        raise SystemExit(f"missing {p}")
    u = {
        "id": uid,
        "source": repo_name,
        "platform": "incident-repro" if tier == "X" else "secure-path-test",
        "tier": tier,
        "kind": kind,
        "files": [{
            "kind": "repo-file",
            "repo": repo_url,
            "commit": commit,
            "path": path,
            "sha256_norm": norm_hash(p.read_text(encoding="utf-8", errors="replace")),
        }],
        "group_id": None,
        "group_size": None,
        "duplicate_of": None,
        "license": "none",
        "unlicensed": True,
    }
    if real_incident is not None:
        u["real_incident"] = real_incident
    if note:
        u["note"] = note
    return u


def x_units() -> list[dict]:
    units = []
    dhl = "SunWeb3Sec-DeFiHackLabs"
    dhl_commit = git_head(CACHE / "repos" / dhl)
    for path in [
        "src/test/2026-10/EtherVista_exp.sol", "src/test/2026-11/Aramid_exp.sol",
        "src/test/2026-11/ChainGames_exp.sol", "src/test/2026-11/GDC_exp.sol",
        "src/test/2026-11/MOREDeFi_exp.sol", "src/test/2026-11/Premia_exp.sol",
        "src/test/2026-11/TENDRIL_exp.sol", "src/test/2026-11/Unifund_exp.sol",
        "src/test/2026-11/sBOLD_exp.sol",
    ]:
        units.append(repo_file_unit(f"X-dhl-{Path(path).stem}", "X", dhl,
                                    "https://github.com/SunWeb3Sec/DeFiHackLabs", dhl_commit,
                                    path, "test-file", True))
    imm = "immunefi-team-bugfix-reviews-pocs"
    imm_commit = git_head(CACHE / "repos" / imm)
    for p in sorted((CACHE / "repos" / imm).glob("test/**/*.t.sol")):
        rel = p.relative_to(CACHE / "repos" / imm).as_posix()
        units.append(repo_file_unit("X-imm-" + re.sub(r"[^a-z0-9]+", "-", p.relative_to(CACHE / "repos" / imm).as_posix().lower()).strip("-"), "X", imm,
                                    "https://github.com/immunefi-team/bugfix-reviews-pocs", imm_commit,
                                    rel, "test-file", True))
    for repo_name, commit, url, tag in [
        ("faheelsattar-damn-vuln-defi-foundry", git_head(CACHE / "repos" / "faheelsattar-damn-vuln-defi-foundry"),
         "https://github.com/faheelsattar/damn-vuln-defi-foundry", "dvdefi"),
        ("catcatcat8-mixbytes-ctf", git_head(CACHE / "repos" / "catcatcat8-mixbytes-ctf"),
         "https://github.com/catcatcat8/mixbytes-ctf", "mixbytes"),
    ]:
        for p in sorted((CACHE / "repos" / repo_name).glob("test/**/*.t.sol")):
            rel = p.relative_to(CACHE / "repos" / repo_name).as_posix()
            units.append(repo_file_unit("X-" + tag + "-" + re.sub(r"[^a-z0-9]+", "-", p.relative_to(CACHE / "repos" / repo_name).as_posix().lower()).strip("-"), "X", repo_name, url, commit,
                                        rel, "test-file", False,
                                        "CTF solve: exploit-by-construction teaching test"))
    return units


def n_units(cb: dict) -> list[dict]:
    units = []
    wc = "c4-code-2024-08-wildcat"
    wcommit = git_head(CACHE / "repos" / wc)
    slug = "2024-08-wildcat"
    cb[slug] = {"repo": f"https://github.com/code-423n4/{slug}", "commit": wcommit,
                "source": "c4-sponsor-snapshot"}
    for p in sorted((CACHE / "repos" / wc).glob("test/**/*.t.sol")):
        rel = p.relative_to(CACHE / "repos" / wc).as_posix()
        u = repo_file_unit("N-wildcat-" + re.sub(r"[^a-z0-9]+", "-", rel.lower()).strip("-"),
                           "N", wc, f"https://github.com/code-423n4/{slug}", wcommit,
                           rel, "test-file", None, "secure-path test of the audited contest code")
        u["codebase_id"] = slug
        units.append(u)
    salty_commit = (CACHE / "salty-commit.txt").read_text(encoding="utf-8").strip()
    slug = "2024-01-salty"
    cb[slug] = {"repo": f"https://github.com/code-423n4/{slug}", "commit": salty_commit,
                "source": "c4-sponsor-snapshot"}
    for p in sorted((CACHE / "raw" / "salty").glob("*.sol")):
        path = p.name.replace("src_dao_tests_", "src/dao/tests/") \
                      .replace("src_launch_tests_", "src/launch/tests/")
        units.append({
            "id": "N-salty-" + re.sub(r"[^a-z0-9]+", "-", p.stem.lower()).strip("-"),
            "source": "salty-raw",
            "platform": "secure-path-test",
            "tier": "N",
            "kind": "test-file",
            "files": [{
                "kind": "raw-file",
                "repo": f"https://github.com/code-423n4/{slug}",
                "commit": salty_commit,
                "path": path,
                "url": f"https://raw.githubusercontent.com/code-423n4/{slug}/{salty_commit}/{path}",
                "sha256_norm": norm_hash(p.read_text(encoding="utf-8", errors="replace")),
            }],
            "group_id": None, "group_size": None, "duplicate_of": None,
            "codebase_id": slug,
            "license": "none", "unlicensed": True,
        })
    return units


# ---------------------------------------------------------------------------
# CodeHawks EVM and Hats Finance: fresh judged Solidity findings with Foundry
# test code. Both use the content-based block selector below (never the fence
# label): non-code blocks are rejected, the language is inferred, and a block
# is eligible only when it carries a Foundry test marker.
# ---------------------------------------------------------------------------
CODEHAWKS = CACHE / "codehawks"
HATS = CACHE / "hats"
ALLFENCE = re.compile(r'```([A-Za-z0-9_+-]*)[ \t]*\n(.*?)```', re.S)
SOLTEST = re.compile(
    r"function\s+test\w*\s*\(|function\s+invariant_\w*\s*\("
    r"|vm\.(?:prank|deal|store|warp|startPrank)"
    r"|assertEq\(|assertTrue\(|assertLt\(|assertGt\("
)
SOLMARK = re.compile(r"pragma solidity|\bcontract\s+[A-Za-z_]\w*|\buint\d*\b|\baddress\b|msg\.sender")
TSMARK = re.compile(r"\b(?:const|let|await)\b|\bit\s*\(|\bdescribe\s*\(")
DIFFLINE2 = re.compile(r"^(?:---|\+\+\+|@@)", re.M)
LOGLINE2 = re.compile(r"^\s*(?:running \d+ tests?|test .* \.\.\. ok|test result:|Compiling )", re.M)
TOML_LINE3 = re.compile(
    r"^\s*(?:\[[A-Za-z0-9_.-]+\]|[A-Za-z0-9_.\"-]+\s*=\s*(?:\"[^\"]*\"|'[^']*'|\d[\w.]*|true|false))\s*$"
)
SHELLCMD3 = re.compile(
    r"^\s*(?:\$\s+)?(?:cd|ls|cat|curl|wget|git|npm|yarn|pnpm|npx|cargo|forge|node"
    r"|chmod|mkdir|export|source|cp|mv|rm|echo|test)\b"
)


def all_blocks(desc: str) -> list[tuple[int, str, str]]:
    return [(n, m.group(1) or "", m.group(2)) for n, m in enumerate(ALLFENCE.finditer(desc or ""))]


def reject_sol(code: str) -> str | None:
    lines = [l for l in code.split("\n") if l.strip()]
    if not lines:
        return "empty"
    if DIFFLINE2.search(code):
        return "diff"
    if LOGLINE2.search(code):
        return "test-log"
    if all(TOML_LINE3.match(l) for l in lines):
        return "toml"
    shell = sum(1 for l in lines if SHELLCMD3.match(l))
    if shell >= 2 and shell >= len(lines) - 1:
        return "shell"
    return None


def select_sol_block(desc: str) -> dict | None:
    cands = []
    for idx, fence_label, code in all_blocks(desc):
        if reject_sol(code) is not None:
            continue
        if not SOLTEST.search(code):
            continue
        if TSMARK.search(code.replace("msg.sender", "")):
            continue
        if not SOLMARK.search(code):
            continue
        cands.append((idx, fence_label, code))
    if not cands:
        return None
    full = [c for c in cands if re.search(r"pragma solidity|\bcontract\s+[A-Za-z_]\w*", c[2])]
    c = (full or cands)[0]
    return {"block": c[0], "fence_label": c[1], "lang": "solidity", "code": c[2]}


def codehawks_units(cb: dict) -> list[dict]:
    meta = json.loads((CODEHAWKS / "meta.json").read_text(encoding="utf-8"))
    units = []
    for slug, m in sorted(meta.items()):
        page = (CODEHAWKS / m["file"]).read_bytes()
        text = page.decode("utf-8", "replace")
        i = text.find("findings.getFindingOverviewsForCompetition")
        k = text.find(">", i)
        e = text.find("</script>", k)
        res = json.loads(json.loads(text[k + 1:e])["body"])[0]["result"]["data"]
        groups = (res.get("highs") or []) + (res.get("mediums") or []) + (res.get("lows") or [])
        if m.get("commit"):
            cb[slug] = {"repo": m["github_url"], "commit": m["commit"],
                        "source": "codehawks-contest-repo"}
        for gi, g in enumerate(groups):
            issues = g.get("issues") or []
            byid = {it["id"]: it for it in issues}
            order = [g.get("selectedIssueId")] + [it["id"] for it in issues if it["id"] != g.get("selectedIssueId")]
            chosen = None
            for iid in order:
                it = byid.get(iid)
                if not it:
                    continue
                sel = select_sol_block(it.get("description") or "")
                if sel:
                    chosen = (iid, it, sel)
                    break
            if chosen is None:
                continue
            iid, it, sel = chosen
            sev = g["severity"].capitalize()
            units.append({
                "id": f"J-ch-evm-{slug}-g{gi}",
                "source": "codehawks-" + slug,
                "platform": "codehawks-evm",
                "tier": "J",
                "kind": "test-fragment",
                "files": [{
                    "kind": "ch-page-json-block",
                    "url": m["wayback"],
                    "page_sha256": m["page_sha256"],
                    "group": g["id"],
                    "issue_id": iid,
                    "block": sel["block"],
                    "lang": sel["lang"],
                    "fence_label": sel["fence_label"],
                    "block_sha256": norm_hash(sel["code"]),
                }],
                "outcome": "valid",
                "severity": sev,
                "severity_title": None,
                "severity_conflict": False,
                "severity_source": "results-group-severity-field",
                "outcome_source": "codehawks-results",
                "outcome_evidence": f'Results page heading "{sev} Risk Findings"; group of {len(issues)} submissions',
                "outcome_url": m["wayback"],
                "group_id": g["id"],
                "group_size": len(issues),
                "duplicate_of": None,
                "contest_submissions": len(issues),
                "codebase_id": slug if m.get("commit") else None,
                "license": "none",
                "unlicensed": True,
            })
    return units


def hats_units() -> list[dict]:
    repo = "hats-finance/Euro-Dollar-0xa4ccd3b6daa763f729ad59eae75f9cbff7baf2cd"
    issues = []
    for p in sorted(HATS.glob("*.json")):
        d = json.loads(p.read_text(encoding="utf-8"))
        if isinstance(d, list):
            issues += d
    units = []
    seen = set()
    for it in issues:
        n = it["number"]
        if n in seen:
            continue
        seen.add(n)
        labs = [l["name"] for l in it["labels"]]
        low = [l.lower() for l in labs]
        if "duplicate" in low or "invalid" in low:
            continue
        sev = next((l.capitalize() for l in labs if l.lower() in ("high", "medium", "low")), None)
        if sev is None:
            continue
        sel = select_sol_block(it.get("body") or "")
        if sel is None:
            continue
        units.append({
            "id": f"J-hats-eurodollar-{n}",
            "source": "hats-euro-dollar",
            "platform": "hats-finance",
            "tier": "J",
            "kind": "test-fragment",
            "files": [{
                "kind": "gh-issue-block",
                "repo": repo,
                "issue": n,
                "url": it["html_url"],
                "body_sha256": hashlib.sha256((it.get("body") or "").encode("utf-8")).hexdigest(),
                "block": sel["block"],
                "lang": sel["lang"],
                "fence_label": sel["fence_label"],
                "block_sha256": norm_hash(sel["code"]),
            }],
            "outcome": "valid",
            "severity": sev,
            "severity_title": None,
            "severity_conflict": False,
            "severity_source": "hats-issue-labels",
            "outcome_source": "hats-issue-labels",
            "outcome_evidence": "Hats labels: " + ", ".join(labs),
            "outcome_url": it["html_url"],
            "group_id": None,
            "group_size": 1,
            "duplicate_of": None,
            "codebase_id": None,
            "codebase_note": "the Hats repository names no audited codebase repo",
            "note": "Hats marks duplicates without a primary link; duplicate-labelled issues are not units",
            "license": "none",
            "unlicensed": True,
        })
    return units

def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args(argv[1:])
    cb = codebases()
    units = c4_units(cb) + codehawks_units(cb) + hats_units() + x_units() + n_units(cb)
    ids = [u["id"] for u in units]
    if len(ids) != len(set(ids)):
        dup = sorted({i for i in ids if ids.count(i) > 1})
        raise SystemExit(f"duplicate unit ids: {dup[:5]}")
    seen = {}
    for u in units:
        key = u["files"][0].get("block_sha256") or u["files"][0].get("sha256_norm")
        u["dedupe_key"] = key
        if key in seen:
            u["content_duplicate_of"] = seen[key]
        else:
            seen[key] = u["id"]
        u["split"] = split_of(u["id"])
    counted = [u for u in units if "content_duplicate_of" not in u]
    by_tier = {}
    for u in counted:
        by_tier[u["tier"]] = by_tier.get(u["tier"], 0) + 1
    j = [u for u in counted if u["tier"] == "J"]
    j_by_source = {}
    for u in j:
        j_by_source[u["source"]] = j_by_source.get(u["source"], 0) + 1
    j_by_platform = {}
    for u in j:
        j_by_platform[u["platform"]] = j_by_platform.get(u["platform"], 0) + 1
    covered = sum(1 for u in j if u.get("codebase_id"))
    doc = {
        "schema": "realism-gate.solidity-poc-inventory/v1",
        "date": "2026-11-25",
        "salt": SALT,
        "split_rule": "int(sha256(salt + '|' + unit_id)[:8], 16) % 100: 0-39 DEV, 40-59 V1, 60-74 V2, 75-99 FINAL",
        "unit_rule": "J: one Code4rena report section (one judged finding group). Duplicate submissions are not separate units; contest_submissions records the curated issue JSONs. X: one incident/teaching exploit test file. N: one secure-path test file.",
        "codebases": cb,
        "units": units,
        "counts": {
            "units": len(units),
            "content_duplicates": len(units) - len(counted),
            "counted_by_tier": dict(sorted(by_tier.items())),
            "J_by_source": dict(sorted(j_by_source.items())),
            "J_by_platform": j_by_platform,
            "J_source_share_pct": {k: round(100.0 * v / len(j), 2) for k, v in sorted(j_by_source.items())} if j else {},
            "J_platform_share_pct": {k: round(100.0 * v / len(j), 2) for k, v in sorted(j_by_platform.items())} if j else {},
            "J_codebase_pinned": covered,
            "J_codebase_null": len(j) - covered,
            "split": {s: len([u for u in counted if u["split"] == s]) for s in ("DEV", "V1", "V2", "FINAL")},
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
