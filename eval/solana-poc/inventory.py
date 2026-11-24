#!/usr/bin/env python3
"""Builds eval/solana-poc/inventory.json: every candidate PoC from the sources in
corpus.json, with tier, kind, include/exclude and the reason, the judged
outcome where one exists, and a dedupe group.

The decisions are written here, in SPEC, as data. The script only adds what it
can compute: the normalised sha256 of each file (CRLF -> LF, trailing spaces
stripped, leading/trailing blank lines dropped), the dedupe groups, and the
counts. A unit whose files hash the same as an earlier unit's is a duplicate:
it stays in the inventory with `duplicate_of` and is not run or counted.

Extracted units (PoCs that live in a markdown issue or a report page) are
hashed from the work tree that extract.py writes; run extract.py first.

Usage:
  eval/solana-poc/inventory.py            write inventory.json
  eval/solana-poc/inventory.py --check    verify inventory.json matches
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solana-poc"
WORK = ROOT / ".corpus" / "solana-poc-work"

# Kinds of PoC artifact. RQ1 reports readability per kind.
KINDS = {
    "test-file": "a test file (Rust #[test]/#[tokio::test] or TS it/test) meant to drive a harness",
    "test-fragment": "statements or an it() block pasted into an issue, meant to be added to an existing test",
    "rust-unit-test": "a Rust #[test] that calls program functions directly, driving no Solana harness",
    "script": "a TS/JS script run with node (no it/test), e.g. a CTF client or a replaced deploy script",
    "binary-main": "a Rust binary with fn main (PoC framework or CTF client)",
    "ctf-solve-program": "an on-chain attacker program (entrypoint), not a test",
}

# Outcome sources for the judged tier.
# platform-report: the platform's published report (Sherlock README/report, Code4rena report).
# self-reported: the author's repository states it was approved; used only in an RQ3 sensitivity row.

def U(uid, source, tier, kind, files, **kw):
    d = {"id": uid, "source": source, "tier": tier, "kind": kind, "files": files}
    d.update(kw)
    return d


ORDERLY_TESTS = "solana-vault/packages/solana/contracts/tests"
ORDERLY_SCRIPTS = "solana-vault/packages/solana/contracts/scripts"
ORDERLY_REPORT = "README.md of sherlock-audit/2024-09-orderly-network-solana-contract-judging (the published Sherlock report: H-1 #40, H-2 #99, M-1 #146)"
WOOFI_REPORT = "README.md of sherlock-audit/2024-08-woofi-solana-deployment-judging (the published Sherlock report: H-1 #29, H-2 #64, M-1 #13, M-2 #54, M-3 #73)"
PUMP_REPORT = "https://code4rena.com/reports/2025-01-pump-science"
W3M = "repository description of web3-master/audit-reports: 'All submitted and approved issues at code4rena, sherlock and immunefi.'"

SPEC_UNITS = [
    # ---------------- Tier J: judged contest PoCs ----------------
    U("J-orderly-078", "orderly-judging", "J", "test-fragment",
      [{"extract": "orderly-judging", "md": "001/078.md", "block": 0, "host_source": "orderly-code",
        "host_dir": ORDERLY_TESTS, "name": "sherlock-078.ts"}],
      outcome="valid", outcome_source="platform-report",
      outcome_evidence=f"in duplicate family 001 whose lead #40 is H-1 in the {ORDERLY_REPORT}; author handle chinepun is in H-1 'Found by'",
      placement="issue says 'Add' with no file; placed in the code repo's tests/ directory"),
    U("J-orderly-080", "orderly-judging", "J", "test-fragment",
      [{"extract": "orderly-judging", "md": "001/080.md", "block": 0, "host_source": "orderly-code",
        "host_dir": ORDERLY_TESTS, "name": "sherlock-080.ts"}],
      outcome="valid", outcome_source="platform-report",
      outcome_evidence=f"in duplicate family 001 (lead #40 = H-1) of the {ORDERLY_REPORT}; author chinepun credited",
      placement="issue says 'Add' with no file; placed in tests/"),
    U("J-orderly-066", "orderly-judging", "J", "rust-unit-test",
      [{"extract": "orderly-judging", "md": "006/066.md", "block": 0, "name": "sherlock-066.rs"}],
      outcome="invalid", outcome_source="platform-report",
      outcome_evidence=f"family 006 has no issue in the {ORDERLY_REPORT}",
      placement="a #[cfg(test)] mod for msg_codec.rs; written standalone"),
    U("J-orderly-085", "orderly-judging", "J", "script",
      [{"extract": "orderly-judging", "md": "014/085.md", "block": 0, "host_source": "orderly-code",
        "host_dir": ORDERLY_SCRIPTS, "name": "05deposit_vault.ts"}],
      outcome="invalid", outcome_source="platform-report",
      outcome_evidence=f"family 014 has no issue in the {ORDERLY_REPORT}",
      placement="issue says 'replace 05deposit_vault.ts'; written over scripts/05deposit_vault.ts in the work copy"),
    U("J-orderly-140", "orderly-judging", "J", "script",
      [{"extract": "orderly-judging", "md": "014/140.md", "block": 0, "host_source": "orderly-code",
        "host_dir": ORDERLY_SCRIPTS, "name": "05deposit_vault.ts"}],
      outcome="invalid", outcome_source="platform-report",
      outcome_evidence=f"family 014 has no issue in the {ORDERLY_REPORT}",
      placement="as 085: replaces scripts/05deposit_vault.ts"),
    U("J-orderly-112", "orderly-judging", "J", "rust-unit-test",
      [{"extract": "orderly-judging", "md": "112.md", "block": 0, "name": "sherlock-112.rs"}],
      outcome="invalid", outcome_source="platform-report",
      outcome_evidence=f"root-level issue (no family), not in the {ORDERLY_REPORT}",
      placement="unit tests for the message codec; written standalone"),
    U("J-orderly-115", "orderly-judging", "J", "rust-unit-test",
      [{"extract": "orderly-judging", "md": "115.md", "block": 0, "name": "sherlock-115.rs"}],
      outcome="invalid", outcome_source="platform-report",
      outcome_evidence=f"root-level issue, not in the {ORDERLY_REPORT}",
      placement="written standalone"),
    U("J-woofi-070", "woofi-judging", "J", "test-fragment",
      [{"extract": "woofi-judging", "md": "070.md", "block": 1, "host_source": "woofi-code",
        "host_dir": "WOOFi_Solana/tests", "name": "sherlock-070.js"}],
      outcome="invalid", outcome_source="platform-report",
      outcome_evidence=f"root-level issue, not in the {WOOFI_REPORT}",
      placement="an it() block with no file named; placed in WOOFi_Solana/tests/"),
    U("J-pump-H02", "pump-science", "J", "rust-unit-test",
      [{"extract": "report:pump-science", "anchor": "h-02-missing-update-of-migration_token_allocation-on-global-struct",
        "block": 0, "name": "c4-pump-H02.rs"}],
      outcome="valid", outcome_source="platform-report",
      outcome_evidence=f"H-02 in {PUMP_REPORT}",
      placement="a #[test] for state/global.rs; written standalone"),
    U("J-pump-M03", "pump-science", "J", "script",
      [{"extract": "report:pump-science", "anchor": "m-03-abrupt-fee-transition-from-876-to-1-at-slot-250-due-to-incorrect-linear-decrease-formula",
        "block": 1, "name": "c4-pump-M03.ts"}],
      outcome="valid", outcome_source="platform-report",
      outcome_evidence=f"M-03 in {PUMP_REPORT}",
      placement="a TypeScript fee simulation; written standalone"),
    U("J-w3m-onre", "web3-master-audit-reports", "J", "test-file",
      [{"path": "immunefi/2026-05-onre/PoC.ts"}],
      outcome="valid", outcome_source="self-reported", outcome_evidence=W3M,
      note="imports ../test_helper and ../onre_program.ts, which the portfolio does not contain"),
    U("J-w3m-chainlink", "web3-master-audit-reports", "J", "test-file",
      [{"path": "immunefi/2026-05-chainlink/poc.spec.ts"}],
      outcome="valid", outcome_source="self-reported", outcome_evidence=W3M),

    # ---------------- Tier X: exploit-by-construction, sources named in the prompt ----------------
    *[U(f"X-ackee-l6-{n}", "ackee-bootcamp", "X", "test-file", [{"path": f"Lesson-6/{n}/tests/{n}.ts"}])
      for n in ["account-reloading", "arbitrary-cpi", "duplicate-mutable-accounts", "pda-privileges",
                "re-initialization", "revival-attack", "signer-authorization"]],
    *[U(f"X-taented9-{n[:2]}", "solana-security-patterns", "X", "test-file",
        [{"path": f"examples/{n}/tests/exploit.ts"}], pair=f"NC-taented9-{n[:2]}")
      for n in ["01-missing-account-validation", "02-signer-authorization", "03-arithmetic-overflow",
                "04-cpi-security", "05-account-closure"]],
    U("X-otter-wallet-king", "ottersec-devcave-2025", "X", "ctf-solve-program",
      [{"path": "pwn/wallet-king/solve/src/lib.rs"}]),
    U("X-otter-e2e-nft", "ottersec-devcave-2025", "X", "ctf-solve-program",
      [{"path": "pwn/e2e-nft-trading/solve/src/lib.rs"}]),
    U("X-neo-cashio", "neodyme-solana-ctf", "X", "binary-main",
      [{"path": "cashio-exploit-workshop/poc/src/main.rs"}, {"path": "cashio-exploit-workshop/poc/src/lib.rs"}]),
    U("X-neo-bugchain-author", "neodyme-solana-ctf", "X", "binary-main",
      [{"path": "allesctf21/bugchain/solution/author/src/main.rs"},
       {"path": "allesctf21/bugchain/solution/author/program/src/lib.rs"}]),
    U("X-neo-legitbank-author", "neodyme-solana-ctf", "X", "binary-main",
      [{"path": "allesctf21/legit-bank/solution/author/src/main.rs"}]),
    U("X-neo-legitbank-sintemal", "neodyme-solana-ctf", "X", "binary-main",
      [{"path": "allesctf21/legit-bank/solution/sintemal/solution/src/main.rs"},
       {"path": "allesctf21/legit-bank/solution/sintemal/bank_initializer/src/lib.rs"}]),
    U("X-neo-pool-solve", "neodyme-solana-ctf", "X", "binary-main",
      [{"path": "pool/client/framework-solve/src/main.rs"},
       {"path": "pool/client/framework-solve/solve/programs/solve/src/lib.rs"}]),
    *[U(f"X-solhana-ch{i}", "solhana-ctf", "X", "script",
        [{"path": f"client/challenge{i}.js"}, {"path": "client/util.js"}, {"path": "client/api.js"}])
      for i in (1, 2, 3)],

    # ---------------- Tier X: GitHub code-search screen ----------------
    # One unit per hit file the screen included; see SCREEN for the rest.
]

CS_INCLUDED = {
    "cs-0xsh1mm3r--solanaplayground": ["01-signer-check/poc/tests/exploit.rs", "02-owner-check/poc/tests/exploit.rs",
                                       "03-account-substitution/poc/tests/exploit.rs", "04-share-vault/poc/tests/exploit.rs",
                                       "06-redeem-vault/poc/tests/exploit.rs"],
    "cs-alphar2--soteria": ["programs/amm/amm-vulnerable/tests/integration.rs",
                            "programs/governance/g-vulnerable/tests/integration.rs",
                            "programs/pino-escrow/p-vulnerable/tests/exploit_double_take.rs",
                            "programs/pino-escrow/p-vulnerable/tests/exploit_fake_offer.rs",
                            "programs/pino-escrow/p-vulnerable/tests/exploit_missing_signer.rs",
                            "programs/pino-escrow/p-vulnerable/tests/exploit_wrong_proposer.rs"],
    "cs-ansh-699--poc": ["tests/poc1_unauthorized_oracle_creation.ts"],
    "cs-cybwithflourish--vulnerable.so": ["scripts/attacks/04-cpi-reentrancy.ts"],
    "cs-captainlevi2k04--solaudits": ["capstone_bootcamp_4/tests/CollateralFactor-Bug.ts",
                                      "capstone_bootcamp_4/tests/UserDeposit-Bug.ts"],
    "cs-e-okelloh--praxis": ["tests/e2e/tests/escrow.rs"],
    "cs-justuzair--audits": ["contests/others/rektoff-metalend/tests/meta-lend.ts"],
    "cs-kartikvyas1604--safemask": ["examples/solana-demo.ts"],
    "cs-nishantcoder108--rust-in-practice": ["experiment/rektoff/capstone_bootcamp_3_/tests/meta-lend.ts"],
    "cs-parthrathix0--verified-anchor": ["rust/verified-anchor/tests/runtime_exploits.rs"],
    "cs-seann2003--lending-audit": ["tests/meta-lend.ts"],
    "cs-jmcastellanojimenez--rektoff-capstone-project": ["tests/meta-lend.ts"],
    "cs-zsh28--rektoff": ["capstone/tests/meta-lend-exploits.ts"],
    "cs-georgeklein--scale-amm": ["tests/fee-rounding-exploit-tests.ts"],
    "cs-hifey62--solana-security-pitfalls": ["examples/01-missing-signer-check/tests/missing-signer-check.ts"],
    "cs-jcabrera702--solana-exploit-lab": ["tests/challenge_0_signer.rs"],
    "cs-nathfavour--bytesunchecked": ["tests/vuln_missing_signer.ts"],
    "cs-onlybolu--solana-security-examples": ["tests/authority-checks.ts"],
    "cs-ruxy1212--solana-100-days-mlh": ["week_12/day_82/attack.rs"],
    "cs-tanishaf28--100-days-of-solana": ["week12/leaky_vault/programs/leaky_vault/tests/attack.rs"],
    "cs-tobiayinmiro23--100-days-of-solana": ["leaky_vault/programs/leaky_vault/tests/attack.rs"],
    "cs-sadiqsaidu--solana-security-patterns": ["programs/01_missing_account_validation/tests/exploit.ts",
                                                "programs/02_missing_authority_check/tests/exploit.ts"],
    "cs-tadeoaguilar--solanalearn": ["07-TestingSecurityAuditing/reference-vulnerable-bank/tests/exploits.ts"],
    "cs-tobysolutions--solana-defi-course": ["builders/anchor/programs/vault-bug/tests/exploit.rs"],
    "cs-tradewithsg--solana-security-examples": ["tests/missing-account-validation.ts"],
    "cs-danielasaboro--solana_badger": [
        "tests/bump-seed-canonicalization.test.ts", "tests/closing-accounts.test.ts", "tests/integer-overflow.test.ts",
        "tests/owner-checks.test.ts", "tests/panic-handling.test.ts", "tests/pda-sharing.test.ts",
        "tests/pinocchio/arbitrary-cpi.pinocchio.test.ts", "tests/pinocchio/closing-accounts.pinocchio.test.ts",
        "tests/pinocchio/integer-overflow.pinocchio.test.ts", "tests/pinocchio/owner-checks.pinocchio.test.ts",
        "tests/pinocchio/reinitialization.pinocchio.test.ts", "tests/pinocchio/signer-checks.pinocchio.test.ts",
        "tests/pinocchio/type-cosplay.pinocchio.test.ts", "tests/pinocchio/unsafe-rust.pinocchio.test.ts",
        "tests/reinitialization-attacks.test.ts", "tests/signer-checks.test.ts", "tests/type-cosplay.test.ts",
        "tests/unsafe-rust.test.ts"],
    "cs-corelia-academy--solana-bootcamp": [f"exercises/security-ctf/{n}/solution/tests/test.rs"
                                            for n in ["course-registry", "savings-vault", "scoreboard", "ticket-refund"]],
    "cs-mm88nl-web--casi-app": ["tests/security-findings.ts"],
    "cs-neocryptoquant--solana-security-template": ["tests/insecure_init.rs", "tests/remaining_accounts.rs"],
    "cs-ubuntu-technologies--solana-security-template": ["tests/insecure_init.rs", "tests/remaining_accounts.rs"],
    "cs-solizardking--clawd-percolator": ["programs/percolator-prog/tests/poc_v17_lp_redemption_stranding.rs"],
}

# RQ4 negative controls: the patched counterpart of an exploit file.
SPEC_CONTROLS = [
    U(f"NC-taented9-{n[:2]}", "solana-security-patterns", "NC", "test-file",
      [{"path": f"examples/{n}/tests/secure.ts"}], pair=f"X-taented9-{n[:2]}")
    for n in ["01-missing-account-validation", "02-signer-authorization", "03-arithmetic-overflow",
              "04-cpi-security", "05-account-closure"]
]

# Candidates looked at and excluded, with the reason. Counted in the dead-ends table.
SPEC_EXCLUDED = [
    # judged tier: issues whose code blocks are program/source quotes or fixes, not PoCs
    *[{"source": "orderly-judging", "path": p, "reason": "code blocks quote program source or the fix; no coded PoC"}
      for p in ["001/093.md", "001/095.md", "002/005.md", "004/129.md", "005/096.md", "011/041.md",
                "018/114.md", "113.md", "151.md", "170.md"]],
    {"source": "orderly-judging", "path": "(other 160 issue files)", "count": 160,
     "reason": "no fenced code block with a test, harness call or PoC marker (screened by grep for program.methods|anchor.workspace|it(|#[test]|describe(|LiteSVM|ProgramTest|bankrun)"},
    {"source": "woofi-judging", "path": "002/058.md", "reason": "code blocks quote program source; no coded PoC"},
    {"source": "woofi-judging", "path": "(other 88 issue files)", "count": 88,
     "reason": "no fenced code block with a test or PoC marker"},
    {"source": "pump-science", "path": "tests/bankrun.ts, tests/pump-science.ts, clients/js/tests/amm.test.ts",
     "count": 3, "reason": "the sponsor's own test suite, not a PoC"},
    {"source": "pump-science", "path": "report: H-01, M-01, M-02 and QA 01-12",
     "reason": "report code blocks quote source or describe a scenario; no coded PoC (H-01's code is a source quote, not a test)"},
    {"source": "web3-master-audit-reports", "path": "defillama/2026-05-project0/report.md",
     "reason": "Solana finding with source quotes only; no coded PoC"},
    {"source": "web3-master-audit-reports", "path": "immunefi/{2026-04-mux,2026-05-compound,2026-05-mux,2026-05-ostium,2026-05-nexusmutual}",
     "count": 5, "reason": "EVM PoCs (ethers/hardhat/viem), not Solana"},
    # tier X: templates and non-PoCs
    *[{"source": "ackee-bootcamp", "path": f"Capture-the-Flag/level-{i}/tests/exploit.ts",
       "reason": "unsolved CTF template: the 'ADD YOUR CODE' slot is empty"} for i in (1, 2, 3, 4, 5)],
    {"source": "ackee-bootcamp", "path": "Lesson-6/ownership-check/tests/ownership-check.ts",
     "reason": "template: it('TODO') with an empty slot"},
    {"source": "ackee-bootcamp", "path": "Lesson-6/type-cosplay/tests/type-cosplay.ts",
     "reason": "template: it('TODO') with an empty slot"},
    {"source": "ackee-bootcamp", "path": "Lesson-6/initialization-frontrunning/tests/initialization-frontrunning.ts",
     "reason": "only secure-path tests (wrong/correct authority on the secure instruction); no exploit test"},
    *[{"source": "ackee-breakpoint-2023", "path": f"exploit-pocs/exploit-{i}/exploit-{i}.ts",
       "reason": "exercise harness: the exploit slot is empty (0, 1) or the exploit is the participant's parameter choice (2, 3); not a solved PoC"}
      for i in (0, 1, 2, 3)],
    {"source": "ottersec-devcave-2025", "path": "pwn/cut-and-run/{,attachments/}program/programs/cut-and-run/tests/solve_base_program_test.rs",
     "count": 2, "reason": "unsolved template: 'you can test your exploit idea here' with no attack between the asserts"},
    {"source": "ottersec-devcave-2025", "path": "pwn/cut-and-run/{,attachments/}program/tests/cut-and-run.ts",
     "count": 2, "reason": "Anchor's default it('Is initialized!') template"},
    {"source": "ottersec-devcave-2025", "path": "pwn/{wallet-king,e2e-nft-trading}/attachments/solve/src/lib.rs",
     "count": 2, "reason": "the handout copies: the solve program is a stripped template (the solved versions are units)"},
    {"source": "sealevel-attacks", "path": "tests/raw.ts",
     "reason": "Anchor's default it('Is initialized!') template; the repository has no exploit tests"},
    *[{"source": "neodyme-workshop", "path": f"pocs/src/bin/level{i}.rs",
       "reason": "unsolved PoC-framework template: hack() is empty"} for i in range(5)],
    {"source": "neodyme-solana-ctf", "path": "neodyme-breakpoint-workshop/pocs/src/bin/level0-4.rs", "count": 5,
     "reason": "byte-identical copies of the neodyme-workshop templates"},
    {"source": "neodyme-solana-ctf", "path": "solhana-ctf/client/challenge1-3.js", "count": 3,
     "reason": "template copies: attack() is empty (the solved versions are in dachen0/solhana-ctf)"},
    {"source": "neodyme-solana-ctf", "path": "pool/client/framework-solve/solve/tests/solve.ts",
     "reason": "Anchor's default it('Is initialized!') template"},
    {"source": "neodyme-solana-ctf", "path": "league-of-lamports/solution_template/{solve.py,solution/src/solution/solution.c}",
     "count": 2, "reason": "solution template, and not Rust/TypeScript"},
    {"source": "neodyme-solana-ctf", "path": "darksols, moar-horse-5, solfire, HalbornCTF_Rust_Solana, allesctf21/secret-store",
     "count": 5, "reason": "challenge sources with no solve in the repository"},
]

# GitHub code search, 2026-11-25: four queries, 181 hits in 92 repositories.
SCREEN = {
    "queries": {
        "LiteSVM attacker exploit language:Rust": 77,
        "ProgramTest attacker exploit language:Rust": 20,
        "bankrun attacker exploit language:TypeScript": 26,
        "anchor.workspace attacker exploit language:TypeScript": 58,
    },
    "hits": 181,
    "repositories": 92,
    "rule": ("Include a hit file when its tests demonstrate an exploit succeeding against a Solana program "
             "(titles or names say exploit/attack/PoC/bug and the test asserts the attack's effect). Exclude: "
             "realism-gate itself; tool, agent, prompt, template or fuzzer source; regression or security suites "
             "where the attack is expected to fail; feature suites; course data; non-Solana code. Only hit files "
             "are taken; sibling files of an included repository are not searched."),
    "excluded_repositories": {
        "KhomDev/realism-gate": "the tool under study",
        "0xdefence-labs/0xDefend": "AI PoC-generator source and its prompts",
        "0xwi11iam/Suijin": "red-team framework source, not a test",
        "rade-nugroho/4R3S": "AI auditor source",
        "solanabr/auditor-skill": "agent skill template",
        "dumbbutt0/audit-watch": "PoC template",
        "Replikanti/agentis-colonies": "agent-colony demo output (repository describes itself as an agent platform)",
        "iamomm-hack/faultline": "replay-tool source",
        "asymmetric-research/crucible": "fuzzer source",
        "sinaabgarmy/AI_Agent_Office": "agent UI source",
        "alrimarleskovar/RoundFinancial": "indexer source",
        "vivekpal1/superteam-academy-lobster-green": "course data file",
        "MetalLegBob/fantastical-finance-factory": "unit tests of constants",
        "aeyakovenko/percolator-meta": "program unit tests",
        "dcccrypto/percolator-stake": "program unit tests and a regression test",
        "Solizardking/clawd-percolator (other hits)": "regression/adversarial suites where the attack fails; processor unit tests",
        "tradewife/percolator-heist": "regression suite",
        "x402agent/openclawd-main": "regression suite (percolator copy)",
        "ChartGains/percolator-app": "regression suite",
        "Ah-Riz/velthoryn": "security suite: every exploit attempt is expected to be blocked",
        "JatinSharma222/Epoch": "security suite: attacks rejected",
        "Lovexan369/drfraudsworth": "security suite: attacks mitigated",
        "OOBE-PROTOCOL/synapse-sap": "security suite: invalid inputs rejected",
        "Oliverknowledge/FairX": "feature/security suite",
        "PlayHyperia/hyperbet": "security suite: attacks rejected",
        "QuantuLabs/8004-atom": "attack-vector coverage suite; outcomes are mitigations",
        "SolCardDev/staking-program": "security suites: attacks rejected",
        "Parad0x-Labs/dna-x402": "regression: drain rejected",
        "Parad0x-Labs/Dark-Null-Protocol": "red-team regression tests; the attack outcome is not stated as succeeding",
        "lfdt-ibc/ibc-contracts": "program unit tests: exploits rejected",
        "dbrowne/example_solana_vault": "security suite: attacks rejected",
        "Axis-pizza/Axis_AMM": "regression suite",
        "tobySolutions/SolanaAMM": "regression suite (Axis copy)",
        "Sufon/TICK": "account-matrix fuzz suite",
        "aizen299/aegis-protocol": "feature/security suite",
        "arabafenice599rae/Token-sperimental": "security suite: attacks rejected",
        "pina-rs/pina": "integration and regression suites",
        "Shawnchee/Whistle": "feature suite",
        "Dodecahedr0x/nebulous-world": "feature suite",
        "Bench-games/opportunity-markets": "feature suite",
        "0xLeathery/the-daily-rug": "feature suite",
        "mihailShumilov/covantic": "feature and regression suites",
        "levy-street/world-of-claudecraft": "game tests, not a Solana harness",
        "mgocbr3/world-of-aphasya": "game tests, not a Solana harness",
        "malbeclabs/doublezero": "feature suite",
        "solana-foundation/subscriptions": "feature suite",
        "solana-program/stake-pool": "feature suite",
        "pointgroup-labs/fogo-stake-pool": "feature suite (stake-pool fork)",
        "OpenAudio/solana-programs": "feature suite; the secp '_exploit' tests assert rejection",
        "chinmaisiddhartha/audius-main": "copy of OpenAudio's suite",
        "dawidosX/ANL-Protocol": "integration suite; attack tests assert rejection",
        "otter-sec/anchor-community": "marginfi suite: attacker attempts should fail",
        "retrovaliks/magnifi-v2": "marginfi suite copy",
        "dikipidorot/AggreMax-marginfi_yield_cpi_integration": "marginfi suite copy",
        "cyranoaladin/formation_web3": "post-patch validation: exploit should be rejected",
        "b3njaminfuentes/Djinn-pmarket": "end-to-end suite; attack phase asserts rejection",
        "oakenknight/solana-school": "Anchor default template",
        "RaYYeR220/deliverable": "audit reproductions converted to regression guards (the file header says the assertions were inverted)",
        "corelia-academy/solana-bootcamp (exercise/ files)": "unsolved exercise templates (the solution/ files are included)",
        "AlphaR2/Soteria (multisig utils.rs)": "helper module, not a test",
        "ParthRathix0/Verified-Anchor (verified-anchor-exploits/src/lib.rs)": "library source, not a test",
        "web3-master/audit-reports": "already a §3 source (chainlink PoC is unit J-w3m-chainlink)",
        "TAENTED9/solana-security-patterns": "already a §3 source",
        "otter-sec/bp-devcave-ctf-2025": "already a §3 source",
    },
}

DEAD_ENDS = [
    {"source": "code-423n4/2025-08-solana-foundation", "count": 0,
     "reason": "the public report has no High or Medium finding and 7 QA reports without coded PoCs; the findings repository is private (404)"},
    {"source": "code-423n4/2025-01-pump-science-findings", "count": 0,
     "reason": "findings repository is private (404); PoCs taken from the public report only, so invalid submissions are not visible"},
    {"source": "Cantina competitions (pump.fun PumpSwap, Olas lockbox-solana)", "count": 0,
     "reason": "findings pages are not readable without login (anonymous fetch returned 404/login on 2026-11-25)"},
    {"source": "dachen0/solhana-ctf server/", "count": 0, "reason": "challenge server, no PoC"},
    {"source": "web3-master/audit-reports outcome labels", "count": 0,
     "reason": "no per-item status; the repository description says all items were approved, so its two Solana PoCs are 'self-reported' outcomes"},
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


def all_units() -> list[dict]:
    units = [dict(u) for u in SPEC_UNITS]
    for source, files in CS_INCLUDED.items():
        for path in files:
            slug = path.replace("/", "_").rsplit(".", 1)[0][-48:]
            units.append(U(f"X-{source[3:]}-{slug}", source, "X", "test-file", [{"path": path}]))
    units += [dict(u) for u in SPEC_CONTROLS]
    return units


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
        # The PoC proper is the first file; helpers listed after it do not make a unit distinct.
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
        "schema": "realism-gate.solana-poc-inventory/v1",
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
        "tier_p_note": "Tier P (the operator's private PoCs) is measured in a private note and never appears in this file.",
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
