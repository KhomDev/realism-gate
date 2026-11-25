#!/usr/bin/env python3
"""Builds eval/solana-poc-v2/inventory.json: every candidate PoC from the v2
sources, with tier, kind, include/exclude and the reason, the judged outcome
where one exists, and a dedupe group.

This is the fresh, sealed corpus for the P1-C re-measurement. It was collected
on 2026-11-25, before any change to the tool's `src/`, and it must not be run
through realism-gate, opened or labelled until the code freeze (see
PREREGISTRATION.md, written at the freeze). Kind is recorded from the file
alone at collection time, so RQ1 is not judged after seeing what the new
readers can do.

The decisions are written here, in ARENA_J / ARENA_X / JITO / PATH_UNITS, as
data. The script only adds what it can compute: the normalised sha256 of each
file (CRLF -> LF, trailing spaces stripped, leading/trailing blank lines
dropped), the dedupe groups, and the counts. A unit whose files hash the same
as an earlier unit's is a duplicate: it stays in the inventory with
`duplicate_of` and is not run or counted.

Extracted units (PoCs that live in a GitHub issue or a report page) are hashed
from the work tree that extract.py writes; run extract.py first.

Usage:
  eval/solana-poc-v2/inventory.py            write inventory.json
  eval/solana-poc-v2/inventory.py --check    verify inventory.json matches
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solana-poc-v2"
WORK = ROOT / ".corpus" / "solana-poc-v2-work"

# Kinds of PoC artifact. RQ1 reports readability per kind.
KINDS = {
    "test-file": "a test file (Rust #[test]/#[tokio::test] or TS it/test) meant to drive a harness",
    "test-fragment": "statements or an it()/#[test] block meant to be added to an existing test",
    "rust-unit-test": "a Rust #[test] that calls program functions directly, driving no Solana harness",
    "script": "a TS/JS script run with node (no it/test), e.g. a CTF client or a replaced deploy script",
    "binary-main": "a Rust binary with fn main (PoC framework or CTF client)",
    "ctf-solve-program": "an on-chain attacker program (entrypoint), not a test",
}


def U(uid, source, tier, kind, files, **kw):
    d = {"id": uid, "source": source, "tier": tier, "kind": kind, "files": files}
    d.update(kw)
    return d


# ---------------------------------------------------------------------------
# Judged: Solana Audit Arena (Frankcastleauditor/Solana-Audit-Arena).
#
# One issue per finding; the maintainer labels every issue (Valid / invalid /
# duplicate + Critical/High/Medium/Low/info) and publishes a final report for
# the week in reports/ of the same repository. The PoC code block is extracted
# from the issue body. The tuple is:
#   (issue, week, block, ext, kind, host_dir)
# Week 1 = stake-flow, 2 = MissionX, 3 = Zenon, 4 = FrankSol,
# 5 = x402-Meridian, 6 = Solana_Autonomix. Issue 233 is titled Week 1 but its
# PoC drives x402-Meridian, so it is placed with Week 5.
#
# Selection is reproducible: the two issues the maintainer marked `invalid`
# and that carry a test block are always kept (RQ3 needs both classes), then
# the 23 remaining labelled issues with a test block are taken in ascending
# sha256(seed|issue) order. The seed is rg-solana-poc-v2-2026-11-25.
# ---------------------------------------------------------------------------
ARENA_J = [
    (140, "4", 2, "rs", "test-file", "FrankSol/programs/yield_generator/tests"),
    (142, "4", 3, "rs", "test-fragment", "FrankSol/programs/yield_generator/tests"),
    (143, "4", 5, "rs", "test-file", "FrankSol/programs/yield_generator/tests"),
    (148, "4", 2, "rs", "test-fragment", "FrankSol/programs/yield_generator/tests"),
    (149, "4", 3, "rs", "test-fragment", "FrankSol/programs/yield_generator/tests"),
    (150, "4", 2, "rs", "test-fragment", "FrankSol/programs/yield_generator/tests"),
    (155, "4", 4, "rs", "test-fragment", "FrankSol/programs/yield_generator/tests"),
    (156, "4", 2, "rs", "test-fragment", "FrankSol/programs/yield_generator/tests"),
    (160, "4", 5, "rs", "test-fragment", "FrankSol/programs/yield_generator/tests"),
    (165, "4", 4, "rs", "test-fragment", "FrankSol/programs/yield_generator/tests"),
    (166, "4", 3, "rs", "test-fragment", "FrankSol/programs/yield_generator/tests"),
    (167, "4", 2, "ts", "test-fragment", "FrankSol/programs/yield_generator/tests"),
    (200, "4", 1, "rs", "test-fragment", "FrankSol/programs/yield_generator/tests"),
    (233, "5", 1, "ts", "test-file", "x402-Meridian/tests"),
    (234, "5", 3, "ts", "test-file", "x402-Meridian/tests"),
    (235, "5", 3, "ts", "test-file", "x402-Meridian/tests"),
    (236, "5", 3, "ts", "test-file", "x402-Meridian/tests"),
    (239, "5", 1, "ts", "test-file", "x402-Meridian/tests"),
    (243, "5", 2, "rs", "rust-unit-test", "x402-Meridian/tests"),
    (244, "5", 1, "ts", "test-file", "x402-Meridian/tests"),
    (246, "5", 1, "rs", "test-file", "x402-Meridian/tests"),
    (249, "5", 6, "ts", "test-file", "x402-Meridian/tests"),
    (255, "5", 12, "ts", "test-fragment", "x402-Meridian/tests"),
    (265, "1", 1, "ts", "test-file", "stake-flow/tests"),
    (275, "5", 3, "ts", "test-file", "x402-Meridian/tests"),
]

# ---------------------------------------------------------------------------
# Exploit-by-construction: Arena submissions whose week has not been judged
# yet (no labels). Same extraction, tier X, no outcome. Selected as the 20
# lowest sha256(seed|issue) among the 134 unlabelled issues that carry a test
# block; issue 278 was dropped after its block turned out to be a compute-unit
# benchmark, not an exploit. Recorded here with kind from the file alone.
# ---------------------------------------------------------------------------
ARENA_X = [
    (9, "1", 1, "ts", "test-file", "stake-flow/tests"),
    (12, "1", 1, "ts", "test-file", "stake-flow/tests"),
    (20, "1", 1, "ts", "test-file", "stake-flow/tests"),
    (25, "1", 1, "ts", "test-file", "stake-flow/tests"),
    (26, "1", 0, "ts", "test-file", "stake-flow/tests"),
    (40, "1", 0, "rs", "test-file", "stake-flow/tests"),
    (49, "2", 5, "ts", "test-file", ""),
    (56, "2", 1, "rs", "test-file", ""),
    (66, "2", 0, "rs", "test-file", ""),
    (99, "3", 5, "ts", "test-file", "Zenon/tests"),
    (170, "4", 5, "rs", "test-fragment", "FrankSol/programs/yield_generator/tests"),
    (186, "4", 3, "rs", "test-file", "FrankSol/programs/yield_generator/tests"),
    (228, "5", 1, "rs", "test-file", "x402-Meridian/tests"),
    (277, "5", 3, "ts", "test-fragment", "x402-Meridian/tests"),
    (279, "5", 2, "ts", "test-file", "x402-Meridian/tests"),
    (289, "6", 2, "rs", "test-fragment", "Solana_Autonomix/relay402/tests"),
    (303, "6", 4, "ts", "test-file", "Solana_Autonomix/relay402/tests"),
    (305, "6", 4, "ts", "test-file", "Solana_Autonomix/relay402/tests"),
    (319, "2", 1, "rs", "test-fragment", ""),
]

# Judged: Immunefi Audit Competition | Jito Restaking. The finding pages are
# rendered HTML outside git, pinned by sha256 in corpus.json. Only findings
# whose published report contains a test block are units; 36675 has no code
# and 36787 quotes program source plus a Token-2022 demonstration test that is
# not the exploit (both excluded below).
JITO = [
    (36903, 1, "rs", "test-fragment", "High"),
    (37079, 5, "rs", "test-fragment", "Insight"),
    (37295, 0, "rs", "test-fragment", "High"),
]

# ---------------------------------------------------------------------------
# Exploit-by-construction binaries and tests from public sources.
# ---------------------------------------------------------------------------
PATH_UNITS = [
    # MerovingioOps/solana-hacks: one PoC binary per vulnerability class,
    # described by the repository as code-level PoCs of bugs named in reports.
    *[U(f"X-merov-{slug}", "merovingio-solana-hacks", "X", "binary-main",
        [{"path": f"{slug}/poc/src/{main}"}])
      for slug, main in [
          ("account-data-match", "main.rs"),
          ("bump-seed-canonicalization", "main.rs"),
          ("cast_truncation", "bin/poc.rs"),
          ("close-account", "main.rs"),
          ("cosplay", "main.rs"),
          ("duplicated", "bin/poc.rs"),
          ("error-not-handled", "main.rs"),
          ("malicious-simulation", "main.rs"),
          ("other-bugs", "main.rs"),
          ("reinitialization", "main.rs"),
      ]],
    # TheGrandPew/jet-governance-pocs: attack files against Jet Governance,
    # written with the OtterSec poc-framework Environment.
    *[U(f"X-jet-adv-{n:02d}", "jet-governance-pocs", "X", "binary-main",
        [{"path": f"os-jet-adv-{n:02d}/src/main.rs"}])
      for n in range(5)],
    # 0xmoostorm/Halborn-Solana-CTF-Solutions: Halborn CTF solve binaries.
    *[U(f"X-halborn-poc{n}", "halborn-ctf-solutions", "X", "binary-main",
        [{"path": f"pocs/src/bin/poc{n}.rs"}])
      for n in (1, 2)],
    # francis-codex/solana-security-patterns: Mollusk exploit/secure pairs;
    # the exploit test is the unit.
    *[U(f"X-francis-p{n:02d}", "francis-codex-patterns", "X", "test-file",
        [{"path": f"patterns/{n:02d}-{slug}/tests/tests/exploit.rs"}])
      for n, slug in [
          (1, "missing-signer-check"), (2, "missing-owner-check"),
          (3, "integer-overflow"), (4, "reinitialization-attack"),
          (5, "pda-bump-canonicalization"), (6, "type-cosplay"),
      ]],
    # Zolldyk/anchor-pinochio-security-template: Anchor TS exploit demos.
    *[U(f"X-zolldyk-p{n:02d}", "zolldyk-anchor-pinocchio", "X", "test-file",
        [{"path": f"patterns/{n:02d}-{slug}/tests/exploit-demo.ts"}])
      for n, slug in [
          (1, "missing-validation"), (2, "authority-checks"),
          (3, "unsafe-arithmetic"), (4, "cpi-reentrancy"),
          (5, "pda-derivation"), (6, "token-validation"),
      ]],
    # Public reproductions and research PoCs.
    U("X-loopscale", "loopscale-hack", "X", "binary-main", [{"path": "src/main.rs"}]),
    U("X-1inch-escrow", "1inch-solana-rescue", "X", "test-file", [{"path": "poc_theft.rs"}]),
    U("X-demo-escrow", "demo-solve-escrow", "X", "test-file",
      [{"path": "native-tests/tests/flow.rs"}]),
    U("X-amm-sandwich", "solana-amm-attacks", "X", "test-file",
      [{"path": "fork/tests/sandwich_e2e.rs"}]),
    U("X-amm-sandwich-program", "solana-amm-attacks", "X", "test-file",
      [{"path": "programs/amm/tests/sandwich_e2e.rs"}]),
]

# ---------------------------------------------------------------------------
# Candidates screened out, with the reason. `count` is the number of candidate
# artifacts the reason covers when it was not enumerated one by one.
# ---------------------------------------------------------------------------
SPEC_EXCLUDED = [
    {"source": "arena", "path": "stake-flow/tests, MissionX/tests, Zenon/tests, FrankSol/**, x402-Meridian/tests, Solana_Autonomix/relay402/tests",
     "count": 9, "reason": "the targets' own feature suites, not PoCs; the Arena units are the issues' PoC blocks"},
    {"source": "arena", "path": "unlabelled issues without a test block",
     "count": 95, "reason": "no fenced block containing a test marker (it()/describe()/#[test]/program.methods)"},
    {"source": "arena", "path": "labelled issues without a test block",
     "count": 13, "reason": "labelled finding but no coded PoC (prose or source quotes only)"},
    {"source": "arena", "path": "issue 278",
     "count": 1, "reason": "the extracted block is a compute-unit benchmark of the shipped vs patched build, not an exploit PoC; excluded after reading the extracted block at collection time"},
    {"source": "lkmidas-web3ctf", "path": "dicectf2023, idekctf2022, sekaictf2023 solve tests",
     "count": 6, "reason": "Anchor default template ('Add your test here' / 'Is initialized!'); the challenge is unsolved"},
    {"source": "paradigm-ctf-2022", "path": "otter-world, otterswap, pool solve tests",
     "count": 6, "reason": "Anchor default template; unsolved"},
    {"source": "de1ik-warrior", "path": "warrior-path-3/ctf-breakpoint-2023/exploit-pocs/exploit-0..3",
     "count": 4, "reason": "unsolved exercise template: the file says 'TASK: Add your code'"},
    {"source": "de1ik-warrior", "path": "warrior-path-3/neodyme-breakpoint-workshop, warrior-path-1/2",
     "count": 19, "reason": "byte-identical copies of P1's neodyme-workshop units, or feature tests, not exploits"},
    {"source": "neodyme-breakpoint-copy", "path": "pocs/src/bin/level0-4.rs",
     "count": 5, "reason": "byte-identical copies of P1's neodyme-workshop units (dedupe against P1)"},
    {"source": "vaultvult-research", "path": "solana-rust-security/*.rs",
     "count": 2, "reason": "print-only simulations with a hardcoded verdict; no program call, no state, no assertion"},
    {"source": "svm-exploit-gym", "path": "tests/rust/tests/smoke.rs",
     "count": 1, "reason": "toolchain smoke test (hello_world), not an exploit"},
    {"source": "princedotrs-audit", "path": "skills/solana-program-audit/templates/**",
     "count": 5, "reason": "agent-skill template and tool source, not a PoC of a specific bug"},
    {"source": "psyto-solinv, lgalabru/masoch", "path": "crates/**",
     "count": 2, "reason": "invariant/fuzzing tool source, not a PoC"},
    {"source": "jito-restaking", "path": "/jito-restaking/36675",
     "count": 1, "reason": "no code block in the published report"},
    {"source": "jito-restaking", "path": "/jito-restaking/36787",
     "count": 1, "reason": "program source with @audit comments plus a Token-2022 demonstration test that does not exercise the vault"},
    {"source": "anchor advisories", "path": "GHSA-c6rc-8jpp-2fgc, GHSA-429q-fhh4-r6hj",
     "count": 2, "reason": "the PoC is program source to build and deploy, not a test; no reader covers it and it is not an on-chain attacker program"},
    {"source": "reports.immunefi.com Firedancer v0.1", "path": "firedancer-v0.1 findings",
     "count": 18, "reason": "validator-client (C) memory-safety PoCs, not Solana program tests; realism-gate has no C frontend"},
    {"source": "code-423n4/2022-02-anchor-findings", "path": "data/*.json",
     "count": 76, "reason": "Anchor framework audit; no issue contains a test or a runnable PoC (prose and source quotes only)"},
    {"source": "code-423n4/2025-08-solana-foundation", "path": "report",
     "count": 0, "reason": "public report has no High/Medium finding and no coded PoC; findings repository private"},
    {"source": "Cantina competitions (Tensor, and others)", "path": "findings",
     "count": 0, "reason": "competition pages do not publish individual findings or PoCs; /findings returned 404 without login"},
    {"source": "code4rena.com/audits 2026-02-jupiter-lend", "path": "repo",
     "count": 0, "reason": "sponsor repository contains only README/scope; no findings or PoCs published yet"},
]

# GitHub code search, run 2026-11-25 with `gh api search/code`, 10 req/min,
# >=7 s spacing. The first four are P1's queries re-run to find remaining hits;
# the fifth is the P1-C addition. Totals are `total_count`; repositories are
# distinct repositories among the first 100 hits. Repositories already screened
# in P1 are not re-screened unless a new file was among the hits.
SCREEN = {
    "queries": {
        "LiteSVM attacker exploit language:Rust": {"total": 77, "repos": 34},
        "ProgramTest attacker exploit language:Rust": {"total": 20, "repos": 14},
        "bankrun attacker exploit language:TypeScript": {"total": 26, "repos": 13},
        "anchor.workspace attacker exploit language:TypeScript": {"total": 58, "repos": 37},
        '"proof of concept" anchor test': {"total": 44565, "repos": 96,
                                            "note": "unscoped phrase query is noise; only the Solana hits were read"},
        "Mollusk exploit language:Rust": {"total": 89, "repos": 63},
        "litesvm attacker language:Rust": {"total": 788, "repos": 55},
        "program_test steal language:Rust": {"total": 41, "repos": 32},
        "poc_framework language:Rust": {"total": 109, "repos": 35},
    },
    "rule": "Include a hit file when its tests demonstrate an exploit succeeding against a Solana program (titles or names say exploit/attack/PoC/bug and the test asserts the attack's effect). Exclude: the tool under study; tool, agent, prompt, template or fuzzer source; regression or security suites where the attack is expected to fail; default Anchor templates; feature suites; print-only simulations; non-Solana code.",
    "included_repositories": {
        "Frankcastleauditor/Solana-Audit-Arena": "judged weekly competition; the units are its labelled and unlabelled issue PoCs",
        "MerovingioOps/solana-hacks": "ten code-level PoC binaries",
        "Zolldyk/anchor-pinochio-security-template": "six Anchor/Pinocchio exploit demos",
        "TheGrandPew/jet-governance-pocs": "five poc-framework attack binaries",
        "0xmoostorm/Halborn-Solana-CTF-Solutions": "two CTF solve binaries",
        "francis-codex/solana-security-patterns": "six Mollusk exploit tests",
        "publicqi/loopscale-hack": "post-mortem reproduction binary",
        "corey888773/solana-amm-attacks-analysis": "two LiteSVM sandwich tests",
        "Tejanadh/1inch-solana-rescue-theft-research": "one program-test PoC",
        "supernewbiecoder/DEMO_solve_escrow": "one program-test demo",
    },
    "excluded_repositories": {
        "VAD37/security-research-archive": "358 MB archive; the Solana entries found are challenge sources and tool output, not solves",
        "scriptCTF/scriptCTF2026-OfficialWriteups": "290 MB; no Solana program PoC in the indexed paths",
        "sajjadium/ctf-archives": "40 GB archive; not sparse-indexable by the search results available here",
        "psyto/solinv": "invariant tool source",
        "lgalabru/masoch": "coverage tool source",
        "princedotrs/solana-audit": "agent skill, templates and tools",
        "svm-exploit-gym": "harness and a toolchain smoke test; no exploit",
        "vaultvult/web3-security-research": "print-only simulations with hardcoded verdicts",
        "Don-Uwe/solana-security-pattern": "repository not found at the recorded path (404) on 2026-11-25",
        "subgroupx": "a web application, not the XCTF Lilac solve repository the write-up appeared to link",
    },
}

DEAD_ENDS = [
    {"source": "Code4rena Solana contests", "count": 0,
     "reason": "the public reports index lists one Solana contest (2025-08-solana-foundation) besides Pump Science; its report has no H/M and no coded PoC, and the 2026-02-jupiter-lend repository holds scope only"},
    {"source": "Sherlock Solana contests", "count": 0,
     "reason": "the org has 462 repositories; only Orderly and WOOFi are Solana, both already fully screened in P1"},
    {"source": "Cantina competitions", "count": 0,
     "reason": "Tensor and the other Solana competitions do not publish individual findings or PoCs to anonymous readers (404 on /findings)"},
    {"source": "Immunefi bug-fix-review PoC repository", "count": 0,
     "reason": "immunefi-team/bugfix-reviews-pocs is Foundry/EVM only; no Solana entry"},
    {"source": "Immunefi Audit Competition reports", "count": 5,
     "reason": "Jito Restaking is the only Solana program competition in the index; its five findings yielded three test blocks (the other two excluded above); Firedancer is a C validator client"},
    {"source": "Hats Finance", "count": 0,
     "reason": "competition findings are delivered to the sponsor, not published per-submission; the Dappslap Solana competition has no public findings report found"},
    {"source": "CodeHawks", "count": 0,
     "reason": "no Solana contest found in the public contest archive on 2026-11-25"},
    {"source": "Security-firm blogs (OtterSec, Neodyme, Zellic, Sec3, Ackee, Halborn, Accretion)", "count": 0,
     "reason": "Zellic/Sec3/Accretion publish report PDFs and post-mortems, not runnable PoC repositories; OtterSec and Neodyme's runnable PoCs are already P1 sources or the poc-framework"},
    {"source": "CTF solve repositories 2023-2026", "count": 0,
     "reason": "the public solves found (lkmidas, paradigm-ctf-2022, De1ik, subgroupx) are unsolved Anchor templates or a mislabelled web app; the solved ones already in P1 (Neodyme, Ackee, OtterSec, solhana) are not reusable without overlap"},
    {"source": "Auditor portfolios", "count": 0,
     "reason": "web3-master and the 1inch/Chainlink-style portfolios used in P1 are exhausted for Solana; vaultvult's PoCs are print-only; Jet Governance PoCs are included as X because no per-finding outcome is published"},
]


def norm_hash(data: bytes) -> str:
    text = data.decode("utf-8", "replace").replace("\r\n", "\n").replace("\r", "\n")
    lines = [l.rstrip() for l in text.split("\n")]
    while lines and not lines[0]:
        lines.pop(0)
    while lines and not lines[-1]:
        lines.pop()
    return hashlib.sha256("\n".join(lines).encode("utf-8")).hexdigest()


def pins() -> dict:
    return json.loads((HERE / "corpus.json").read_text(encoding="utf-8"))


def unit_file_path(unit: dict, f: dict, corpus: dict) -> Path:
    if "extract" in f:
        return WORK / "extract" / unit["id"] / f.get("host_dir", "") / f["name"]
    return ROOT / corpus["corpora"][unit["source"]]["dir"] / f["path"]


def arena_units(corpus: dict) -> list[dict]:
    out = []
    for tier, spec in (("J", ARENA_J), ("X", ARENA_X)):
        for issue, week, block, ext, kind, host in spec:
            pin = corpus["arena_issues"][str(issue)]
            labels = pin["labels"]
            outcome = None
            if "Valid" in labels:
                outcome = "valid"
            if "invalid" in labels:
                outcome = "invalid"
            f = {"extract": "arena-issues", "md": f"{issue}.md", "block": block,
                 "host_dir": host, "name": f"arena-{issue}.{ext}"}
            if host:
                f["host_source"] = "arena"
            unit = U(f"{tier}-arena-{issue}", "arena", tier, kind, [f],
                     week=int(week), issue=issue)
            if tier == "J":
                unit["outcome"] = outcome
                unit["duplicate"] = "duplicate" in labels
                unit["outcome_source"] = "arena-report"
                unit["outcome_evidence"] = (
                    "maintainer labels on the issue (" + "/".join(labels) + "); "
                    "the week's final report is committed in the arena checkout under reports/")
            out.append(unit)
    return out


def jito_units() -> list[dict]:
    out = []
    for issue, block, ext, kind, severity in JITO:
        out.append(U(
            f"J-jito-{issue}", "jito-restaking", "J", kind,
            [{"extract": f"report:jito-{issue}", "block": block, "name": f"jito-{issue}.{ext}"}],
            outcome="valid", outcome_source="platform-report",
            outcome_evidence=(f"Immunefi Audit Competition | Jito Restaking report #{issue}, "
                              f"published severity {severity}"),
        ))
    return out


def all_units() -> list[dict]:
    corpus = pins()
    return arena_units(corpus) + jito_units() + [dict(u) for u in PATH_UNITS]


def build() -> dict:
    corpus = pins()
    units = all_units()
    ids = [u["id"] for u in units]
    if len(ids) != len(set(ids)):
        raise SystemExit("duplicate unit ids")
    seen: dict[str, str] = {}
    for u in units:
        if u["source"] not in corpus["corpora"]:
            raise SystemExit(f"{u['id']}: unknown source {u['source']}")
        hashes = []
        for f in u["files"]:
            p = unit_file_path(u, f, corpus)
            if not p.is_file():
                raise SystemExit(f"{u['id']}: missing {p} (run fetch.sh and extract.py)")
            f["sha256_norm"] = norm_hash(p.read_bytes())
            hashes.append(f["sha256_norm"])
        key = hashes[0]
        u["dedupe_key"] = key
        if key in seen:
            u["duplicate_of"] = seen[key]
        else:
            seen[key] = u["id"]
        lic = corpus["corpora"][u["source"]].get("license", "none")
        u["license"] = lic
        u["unlicensed"] = lic in ("none", "NOASSERTION")
    counted = [u for u in units if "duplicate_of" not in u]
    by_tier: dict[str, int] = {}
    for u in counted:
        by_tier[u["tier"]] = by_tier.get(u["tier"], 0) + 1
    excluded_count = sum(e.get("count", 1) for e in SPEC_EXCLUDED)
    return {
        "schema": "realism-gate.solana-poc-inventory/v2",
        "date": "2026-11-25",
        "kinds": KINDS,
        "units": units,
        "excluded": SPEC_EXCLUDED,
        "code_search_screen": SCREEN,
        "dead_ends": DEAD_ENDS,
        "counts": {
            "units": len(units),
            "duplicates": len(units) - len(counted),
            "counted_by_tier": dict(sorted(by_tier.items())),
            "excluded_candidates": excluded_count,
        },
        "tier_notes": {
            "J": "outcome published by a platform: Sherlock and Code4rena reports (P1), the Solana Audit Arena's maintainer labels and weekly final reports, and Immunefi Audit Competition published reports (Jito Restaking).",
            "X": "exploit-by-construction: CTF solves, teaching PoC binaries, audit reproductions, and Arena submissions whose week is not judged yet. No published outcome.",
            "NC": "none in v2: no new exploit/secure pair was found among the fresh sources.",
        },
    }


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--check", action="store_true")
    args = p.parse_args(argv[1:])
    doc = build()
    text = json.dumps(doc, indent=2, ensure_ascii=False) + "\n"
    out = HERE / "inventory.json"
    if args.check:
        if out.read_text(encoding="utf-8") != text:
            print("inventory.json does not match inventory.py and the checkouts", file=sys.stderr)
            return 1
        print("inventory.json ok")
        return 0
    out.write_text(text, encoding="utf-8", newline="\n")
    print(json.dumps(doc["counts"]))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
