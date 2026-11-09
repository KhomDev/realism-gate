# Benchmark: a public corpus of reproduced exploits

A checker for proofs of concept needs a control set of PoCs that reproduce
something real. The best public one is
[DeFiHackLabs](https://github.com/SunWeb3Sec/DeFiHackLabs) (Apache-2.0): several
hundred Foundry tests, each reproducing an exploit that actually happened.

It is not a set of bug-bounty submissions, and the difference matters when
reading the numbers. A post-incident reproduction is allowed to impersonate a
compromised key, because a key really was compromised. A bounty PoC that does
the same is proving something else. The question for this corpus is therefore
not "how many of these PoCs are wrong" but "does each rule fire on what it
says it fires on, and how often does it fire on something it should not".

## Method

- Corpus: DeFiHackLabs at commit `2c99b565ae24ea2006adf181da20c4419b3edc30`,
  `src/test` only.
- Command: `realism-gate check src/test --no-declaration --format json`, run
  by `scripts/bench-corpus.sh`. CI re-runs it on every pull request and fails
  if any number below moves, so a rule change that shifts them has to update
  this page.
- No declaration, no allow comments, no tuning per file.
- Every finding other than RG08 was read in context by one reviewer (me) and
  classified as correct, debatable, or a false positive. That classification
  is a judgement, not a measurement, and it is in-sample: the corpus was used
  while the rules were written, and two heuristics were adjusted after reading
  it (see "What reading the corpus changed").

## Results

| | |
|---|---:|
| Files | 891 |
| Test suites | 869 |
| Tests | 886 |
| Files the parser could not fully parse | 1 |
| Tests with no finding at all | 248 |
| Tests whose only finding is RG08 | 605 |
| Tests with a finding other than RG08 | 20 in 0.2.0; 30 in 0.5.0; 33 in 0.6.0 |

The machine-readable version is `bench/defihacklabs.json`.

### RG08: most of the corpus asserts nothing

| | Tests | Share |
|---|---:|---:|
| Logs values but asserts none (RG08 warn) | 595 | 67.2% |
| Neither logs nor asserts (RG08 error) | 16 | 1.8% |
| **No assertion of any kind** | **611** | **69.0%** |

This is the corpus's convention, not a defect of each entry: a shared modifier
logs the attacker's balance before and after, and the reader compares the
numbers. It is exactly the habit RG08 exists to break in a bounty PoC, where
the triager should not have to trust a log line.

### Every other finding, reviewed (0.1.0)

This table is the 0.1.0 review. In v0.2.0, RG10 raises 2 findings instead of 5:
the three it dropped all read a balance the test had fixed itself with
`deal`, `vm.deal` or a logging modifier. The labelled re-measurement that
replaces this review is in `eval/` and `docs/precision.md`.

| Rule | Findings | Tests | Correct | Debatable | False positive |
|---|---:|---:|---:|---:|---:|
| RG01 attacker-holds-privilege | 1 | 1 | 1 | 0 | 0 |
| RG02 privileged-step-in-exploit | 15 | 13 | 15 | 0 | 0 |
| RG04 storage-write | 7 | 3 | 7 | 0 | 0 |
| RG05 code-replacement | 4 | 4 | 0 | 1 | 3 |
| RG10 no-baseline | 5 | 5 | 4 | 1 | 0 |
| RG11 unpinned-fork | 1 | 1 | 1 | 0 | 0 |
| **Total** | **33** | **23** | **28** | **2** | **3** |

RG03, RG06, RG07 and RG09 did not fire anywhere in the corpus. That is
expected for RG03 and RG06, which are about harnesses that deploy the protocol
themselves; the corpus forks real chains. RG03 still does not fire here in
0.5.0, as a warning or as a note.

### 0.5.0 counts on the same commit

The historical table above is not rewritten. Rules that did not exist in 0.2.0,
counted on this commit by 0.5.0:

| Rule | Findings | Tests |
|---|---:|---:|
| RG15 profit-ignores-costs | 1 | 1 |
| RG16 implausible-amount | 1 | 1 |
| RG17 unevidenced-setup | 9 | 7 |
| RG18 revert-only | 2 | 2 |

30 tests had a finding other than RG08 in 0.5.0, up from 20. The labelled reading
of these new flags is in [precision](precision.md), not in the 0.1.0 review
table. RG03 is still absent.

### 0.6.0 counts on the same commit

`bench/defihacklabs.json` is the 0.6.0 run. The summary table above matches it.
The 0.5.0 counts were 635 tests with any finding and 30 with a finding other
than RG08.

| | 0.5.0 | 0.6.0 |
|---|---:|---:|
| Tests with any finding | 635 | 638 |
| Tests with a finding other than RG08 | 30 | 33 |
| RG04 findings | 7 (5 error, 2 warn) | 11 (10 error, 1 warn) |

Four new RG04 flags are `stdstore.target(...).checked_write` of a token
`balanceOf` slot. They are labelled in `eval/labels/v0.6.0.jsonl`. RG19 did
not fire: no `setUp` both pranked as a role and wrote storage. The
`balance + 1` shape of RG09 did not fire. A storage write that used to be a
warning because it sat in `setUp` is an error now, except the one the rule
still lowers because the slot names an attacker.

What the correct findings look like, without naming the incidents:

- **RG01.** The harness pranks as the real owner of a helper contract and
  transfers its ownership to the PoC's attack contract. The test's own comment
  calls it "harness setup".
- **RG02.** Fifteen steps run as an owner, a deployer, a keeper, a minter, a
  whitelisted account, a withdrawal-queue contract or a pool manager. The
  comments next to them say things like "quick hack to bypass" an operator
  modifier and "only the owner can call" the function. In a reproduction of a
  key compromise that is the history; in a bounty PoC it is the line a triager
  stops at.
- **RG04.** Storage writes into a rollup contract to restore historical roots,
  into a token to mark an address tax-exempt, and into a token's balance slot.
  The last one funds the attacker and is reported one level lower for that
  reason.
- **RG10.** Four assertions of the form `assertGt(balanceOf(x), N)` where `x`
  could already hold funds on the fork.

The three false positives are all RG05, and all the same shape: the PoC
recreates the attacker's own historical contract, or a chain-specific system
contract, at its original address. The rule cannot tell an attacker's address
from a protocol's when both are bare constants. Its page documents this and
the fix (name the address for what it is).

The two debatable findings are an RG05 that replaces a router's code with a
"readable" equivalent, and an RG10 whose comparison is against a flash-loan
amount rather than a starting balance.

## What reading the corpus changed

Two heuristics were adjusted after the first run, both to remove false
positives, and both changes are covered by unit tests:

1. RG08 initially ignored modifiers, so a test wrapped in a logging modifier
   looked as if it logged nothing. Modifier bodies are now inlined around the
   test body.
2. RG10 initially counted any external call as a state change and treated
   equality assertions as relative claims. It now counts a call written inside
   an assertion or a log as a read, treats a contract the harness deployed as
   starting from zero, ignores checks made before the exploit changes
   anything, and only considers directional assertions.

Those changes are why the numbers are in-sample. An out-of-sample measurement
needs PoCs the rules have never seen, ideally real bounty submissions with
known triage outcomes. If you have such a set and can share counts (not PoCs),
open an issue.

## Reproducing it

```bash
cargo build --release
bash scripts/bench-corpus.sh          # print the summary
bash scripts/bench-corpus.sh --check  # compare with bench/defihacklabs.json
```

The script fetches only `src/test` at the pinned commit into `.corpus/`, which
is gitignored. It takes about two seconds to analyse on a laptop once fetched.
