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


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args(argv[1:])
    cb = codebases()
    units = c4_units(cb) + x_units() + n_units(cb)
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
    j_by_platform = {"code4rena": len(j)}
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
