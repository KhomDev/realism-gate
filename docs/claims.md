# Claims: does the proof prove the claim?

`realism-gate check` still answers one question about the test: did the harness
make the exploit true? 1.0 adds a second question next to it: **does the proof
prove what the report claims?** With `--report`, the tool reads the finding
too, turns its claims into rows, and binds each row to evidence in the PoC. The
output is a claim-by-claim table and, if you ask for them, an
[evidence card](evidence-card.md). There is no score.

```text
CLAIM                           EVIDENCE                                      STATUS
any user, no role               `alice` (makeAddr("alice")) sends the         SHOWN
                                exploit in test_psmDepositBlocked; no role,
                                no prank as protocol (RG01-RG03 clean)
restrict vaults on fee-exempt   not run: `check` never applies a patch. Run   NOT SHOWN
stable collateral to the PSM    `realism-gate verify-fix` on the patch and
                                quote its verdict
```

The row is always a claim the report makes, the evidence the PoC was searched
for, and what was found. A row is not a verdict on the bug. One question only:
did this PoC show this claim.

## Where the claims come from

`check` resolves the claims in this order, and stops at the first one that
exists:

1. `--claims FILE`: the file named on the command line.
2. `claims.yaml` next to `--report`: the same directory as the finding.
3. a YAML front-matter block at the top of the finding.
4. heuristics over the finding's prose, **labelled inferred**.

The first three are **structured**: every field you write is one row, and a
field you leave out is not claimed. The fourth is a fallback and is never a
pass; see [the inferred rule](#the-inferred-rule).

`--claims` works without `--report` when you only want the claim table. The
evidence card is still allowed; its `report` row says "not given" and
`report_sha256` is `null`, because there is no report text to hash.

## The claims.yaml format

The schema is `realism-gate.claims/v1`. It is a flat YAML document, or a
document with a single nested `claims:` key:

```yaml
schema: realism-gate.claims/v1

attacker: any-user
impact: other:distributeSurplus reverts with ReserveBackingShortfall
asset: USDR
amount: the drawn USDR equals the locked stablecoins
preconditions:
  - no privileged role
  - no oracle or price movement
cited:
  - src/VaultEngine.sol:open
  - src/VaultEngine.sol:frob
severity: high
fix: restrict vaults on fee-exempt stable collateral to the PSM
```

Every field is optional except that at least one must be present; a file with
no claims is an error. Unknown keys are rejected, so a typo is an error rather
than a silently missing row. The `schema` key is optional, and when it is
present it has to say `realism-gate.claims/v1`.

| Field | Type | Accepted values | Becomes |
|---|---|---|---|
| `attacker` | string | `any-user`, `role:<NAME>`, `address:<0x...>` (also `unprivileged`, `anyone`, a bare `0x...` address) | one row |
| `impact` | string | `drain`, `freeze`, `halt`, `dos`, `mis-accounting`, or `other:<text>` | one row |
| `asset` | string | free text, e.g. `USDR` | one row |
| `amount` | string | free text, e.g. `drawn USDR equals locked stablecoins` | one row |
| `preconditions` | list of strings | one precondition per entry | one row for the list |
| `cited` | list of strings | `file:function`, a source path, `Contract.function`, or a bare identifier | one row for the list |
| `severity` | string | free text | **no row**: accepted and checked non-empty, not bound (see [below](#severity-is-not-bound)) |
| `fix` | string | free text | one row, always `NOT SHOWN` unless `verify-fix` was run and quoted |

Impact spellings: `drain` also accepts `theft` and `steal`; `freeze` also
accepts `locked`; `halt` also accepts `shutdown`; `dos` also accepts
`denial-of-service`; `mis-accounting` also accepts `misaccounting` and
`accounting`.

### The same block as front matter

The identical YAML can sit at the top of the finding, between `---` lines. If
the finding already has front matter for something else, put the claims under
a `claims:` key; the parser accepts that nesting at the top level, and a
field named both inside and outside the block is an error.

```markdown
---
schema: realism-gate.claims/v1
claims:
  attacker: any-user
  impact: freeze
  asset: USDR
  preconditions:
    - no privileged role
  cited:
    - src/VaultEngine.sol:open
---

# Self-backed vaults switch off a stablecoin's surplus
```

The block has to start on the first line and be closed by the next `---`
line. A horizontal rule later in the body is not a block, and an unclosed
block is not front matter. Unknown keys at the top level are still errors, so
other front-matter keys have to go under their own top-level key: this parser
only knows `claims`.

### Writing a template: `init-claims`

```bash
realism-gate init-claims finding.md --out claims.new.yaml
# wrote claims.new.yaml
```

The template is commented out except for `schema:`, so a fresh file claims
nothing. `init-claims` quotes back the report's `##` headings as precondition
hints and the identifiers it finds in backticks as citation hints; both are
hints to edit, not claims. It refuses to overwrite an existing file without
`--force`, and `--out FILE` writes somewhere other than `claims.yaml` next to
the finding. The exit status is 0 on a write, 2 on a read, write or
already-exists error.

## The predicate table

One row per claim field. Each predicate reads only the parsed PoC (the same
trace, identity facts and rule findings the rules use) and, for `cited`, the
git target tree. Nothing here reads the chain, the network, or the
protocol's documentation.

| Claim field | Evidence predicate: what the PoC must contain | Rule / IR signal | Status when it is missing |
|---|---|---|---|
| `attacker: any-user` | the exploit contains at least one call sent by an address the test controls, and no role was used to reach it | sender replay plus `identity.rs`; `RG01` and `RG02` clean; any privileged sender is read from names, labels, getters and grants | `CONTRADICTED` when `RG01`/`RG02` fired or a privileged sender sends an exploit call; `NOT SHOWN` when `RG04`/`RG05`/`RG06`/`RG19` fired (the state was injected) or no call is made by a controlled address |
| `attacker: role:<NAME>` | nothing: the tool cannot confirm from the test that an attacker holds a role | none | always `NOT CHECKABLE`, with the note to state that role in `realism.yaml` and argue it in the report |
| `attacker: address:<0x...>` | that address sends a call in the exploit | sender replay | `NOT SHOWN` when the address sends no call |
| `impact: drain`, `mis-accounting` | an assertion reads a measured value (a balance, a total supply, another measured quantity) | `reads_in` over the trace; assertion classification | `CONTRADICTED` when the only evidence is a revert assertion: a revert shows the call failed, not that value moved. `NOT SHOWN` when no assertion reads a value |
| `impact: freeze`, `halt`, `dos` | `expectRevert` on the call, plus a sibling test in the same suite that performs one of the same external calls, asserts success and does not expect a revert (the negative control) | `expectRevert`/`expectPartialRevert` assertions; negative control is a sibling `TestFacts` with an overlapping `external_methods` set | `NOT SHOWN` when there is no revert assertion, or when there is one but no sibling negative control: without the control the revert may have nothing to do with the exploit |
| `impact: other:<text>` | the text is read for a verb: a revert word (`revert`, `block`, `freeze`, `ceiling`, `cannot`, ...) selects the revert predicate; a value word (`drain`, `loss`, `transfer`, `withdraw`, ...) selects the value predicate | keyword classification in `bind::impact_kind` | `NOT CHECKABLE` when the text names neither, because there is no predicate to run |
| `asset` | the asset name appears in the PoC's source or in `cited`, **and** an assertion reads a value | source search; measurement assertions | `NOT CHECKABLE` when the PoC never names the asset (the tool cannot tie the impact to it); `NOT SHOWN` when the asset is named but no assertion reads a value |
| `amount` | an assertion compares two different expressions (`assertEq(drawn, locked)`), or a value is read before the exploit changes state and asserted after it | equality assertions; baseline-read detection in the trace | `NOT SHOWN` when no assertion compares two quantities and nothing is read before and after |
| `preconditions` | each entry is classified by its words and checked separately; the worst row decides the field | see the four classes below | per class: see below |
| `preconditions`: privilege or public entry | the words `privileged`, `role`, `admin`, `owner`, `governance`, `minter`, `pauser`; or `any user`, `public`, `permissionless` | `RG01`, `RG02` | `CONTRADICTED` when those rules fired; otherwise `SHOWN` |
| `preconditions`: chain state | the words `oracle`, `price`, `fork`, `chain`, `block`, `snapshot`, `rpc`, `state`, `liquidity` | `RG11`, plus the trace's live-chain flag | `NOT SHOWN` when `RG11` fired or the test reads a fork or live cluster; otherwise `SHOWN` |
| `preconditions`: timing | the words `keeper`, `relayer`, `timing`, `order`, `front-run`, `same block`, `off-chain`, `victim`, `another transaction` | `RG02`, `RG12`, `RG13` | `NOT SHOWN` when those rules fired: someone else has to act; otherwise `SHOWN` |
| `preconditions`: anything else | none: no predicate exists | none | `NOT CHECKABLE` for that entry |
| `cited` | every cited name resolves in the target tree at `--target-commit`: a `file:function`, a source path, a `Contract.function`, or a bare identifier | `cite::Index` (`RG20`'s reader) | `NOT CHECKABLE` without `--target` and `--target-commit`; `NOT SHOWN` listing the names that are not in the tree |
| `fix` | nothing: `check` never applies a patch | none | always `NOT SHOWN`; the evidence says to run `realism-gate verify-fix` and quote its verdict. A fix row that is not shown does **not** make the table incomplete |

A row's status is decided by the predicate, not by prose. The evidence column
quotes or names what was looked at, so a reader can check the predicate's
reasoning against the test.

### Severity is not bound

`severity` is accepted so that a report's own severity claim round-trips
through `init-claims` and does not fail as a typo, and it must be non-empty
when present. It gets no row and no predicate: this tool does not judge
severity, and the rules that ship as errors have their own severities in the
rule catalog.

## Statuses, the outcome line, and what fails

| Status | JSON value | Means |
|---|---|---|
| `SHOWN` | `shown` | the PoC contains the evidence the predicate needs |
| `NOT SHOWN` | `not_shown` | the PoC does not contain it |
| `CONTRADICTED` | `contradicted` | the PoC contains evidence for the opposite of the claim |
| `NOT CHECKABLE` | `not_checkable` | no predicate can be run from source: no target commit, an uncheckable precondition, an unrecognised impact, or a field the report does not state |

The table has a top line. It is `CONTRADICTED` when any row is
`CONTRADICTED`; `INCOMPLETE` when the binding is inferred, or any non-fix row
is not `SHOWN`, or any row is `NOT CHECKABLE`; otherwise `COMPLETE`. The
printed text is one of:

```text
CLAIMS  CONTRADICTED  1 of 2 claims are contradicted by the PoC
CLAIMS  INCOMPLETE  claims inferred
CLAIMS  INCOMPLETE  5 shown, 1 not shown, 1 not checkable
CLAIMS  COMPLETE  every claim the PoC can show is shown; the fix was not run
CLAIMS  COMPLETE  every claim is shown by the PoC
```

## Exit codes

| Exit | When |
|---|---|
| `0` | no active finding is at or above `--fail-on`, no claim is `CONTRADICTED`, and the binding is structured |
| `1` | a finding is at or above `--fail-on`, **or** any claim is `CONTRADICTED`, **or** the binding is inferred |
| `2` | usage, input, configuration or declaration error: an unreadable report or claims file, an invalid claims document, `--card` without `--report`/`--claims`, no test suite found |

Three things about that table are deliberate:

- **`--fail-on` does not suppress a claim failure.** `--fail-on never` turns
  rule findings off; a `CONTRADICTED` claim still exits 1. The claim table is
  about the report holding up, not about a rule's severity.
- **An inferred binding is not a pass**, whatever the rows say. There is no
  exit 0 on inferred claims alone.
- **A non-fix `NOT SHOWN` row does not, by itself, exit 1** in 1.0. The row is
  printed and the outcome says `INCOMPLETE`; the exit code is reserved for a
  contradiction, an inferred binding, and `--fail-on`. If you gate a pipeline
  on every claim being shown, read `claims.outcome` from the JSON, or fail on
  the table yourself.

`--no-fail-on-claims` is the escape hatch. It stops `CONTRADICTED` and
inferred bindings from setting exit 1; the table still prints and the JSON
still says what it says. It exists for a corpus run that scores the statuses
instead of the exit code, not for making a report pass.

The **`RESULT` line follows the same rule** as the exit code. A rule-only run
prints `RESULT PASS` or `RESULT FAIL` for the `--fail-on` level, as before.
When the claims fail, the line says so instead of saying PASS:

```text
RESULT   FAIL  (claims: 1 claim(s) contradicted)
RESULT   FAIL  (claims: claims inferred)
```

That holds even with `--fail-on never`, because the claim failure is not a
rule severity. With `--no-fail-on-claims`, the claims failure is suppressed:
the line stays `RESULT PASS` (or FAIL for a rule finding), the exit status
follows it, and the claim table is unchanged. The `CLAIMS` line always shows
the table's own verdict.

## The inferred rule

When no structured block exists, the reader extracts only what parses
reliably, and says so:

- **cited**: identifiers written in backticks, up to eight, in order,
  duplicates removed. Only shapes that parse: a name, a dotted path, a
  `file:function`, or a source path. Command names such as `cargo`,
  `forge`, `realism-gate` and `vg` are dropped.
- **impact**: the verb in the first heading (or the first non-empty line when
  there is no heading). Only the headline is read, because a verb in the body
  is as likely to describe the fix as the bug.
- **attacker**: the phrases `any user`, `anyone can`, `any account`,
  `unprivileged`, `no privileges`; otherwise a role word in the headline.
- **asset**, **amount**, **preconditions**, **fix**, **severity**: never
  inferred. They come back empty.

A run bound this way gets one row for every field, including the empty ones,
which are printed as `(not stated)` and `NOT CHECKABLE`. The top line is
`INCOMPLETE  claims inferred`, the JSON says `"claim_binding": "inferred"`,
and the exit status is 1. That is the rule: **the tool never returns a pass on
inferred claims alone.** Write a claims block, or a `claims.yaml`, before
treating a table as evidence.

## Worked example: RAIN-USDR

`examples/rain-usdr/` is the RAIN-USDR case study
([khomdev.org](https://khomdev.org/record/rain-usdr-shadow-vaults/)) reduced to a runnable fixture: the finding, a structured
[`claims.yaml`](../examples/rain-usdr/claims.yaml), a Foundry test with the
four behaviours, and the cited contracts as plain source in
[`examples/rain-usdr/protocol/`](../examples/rain-usdr/protocol). The cited contracts
have to be in a git tree for `cited` to bind, so the fixture ships a script
that builds one at a fixed path with a fixed identity and fixed dates:

```bash
cd examples/rain-usdr
sh ./make-target-repo.sh
# TARGET examples/rain-usdr/.target-repo
# COMMIT 5172ee079348b6f56469b148c3808857e1d4d892

realism-gate check test --report finding.md \
  --target .target-repo --target-commit 5172ee079348b6f56469b148c3808857e1d4d892
```

The script prints the commit it made; the commit is the same on every run
because the tree, identity, dates and message are fixed. The run prints the
four RG03 notes (the case study's ordinary local-deployment pattern), then:

```text
CLAIMS  claims.yaml  ·  structured
CLAIM                           EVIDENCE                                      STATUS
any user, no role               `alice` (makeAddr("alice")) sends the         SHOWN
                                exploit in test_psmDepositBlocked; no role,
                                no prank as protocol (RG01-RG03 clean)
other:distributeSurplus         vm.expectRevert(BalanceSheet.ReserveBackingShortfall.selector)  SHOWN
reverts with                    in test_shadowDrawBlocksDistribution;
ReserveBackingShortfall         vm.expectRevert(PegStabilityModule.CeilingExceeded.selector)
(revenue halted) and            in test_psmDepositBlocked; negative control
PSM.sellStable reverts with     test_healthyDistribution_shipsSurplus acts
CeilingExceeded (on-ramp        without the setup and does not revert
denial)
USDR                            `assertEq(drawn, locked)` in                  SHOWN
                                test_shadowDrawBlocksDistribution compares
                                two quantities of `USDR`
the drawn USDR equals the       `assertEq(drawn, locked)` in                  SHOWN
locked stablecoins, so the      test_shadowDrawBlocksDistribution compares
position is capital-neutral     the two quantities the claim is about
no privileged role; no oracle   `no privileged role`: RG01-RG03 clean; `no    SHOWN
or price movement; no keeper    oracle or price movement`: no fork, RPC or
and no off-chain trigger        chain read in the PoC; `no keeper and no
                                off-chain trigger`: every exploit step is
                                the same sender's own call
src/VaultEngine.sol:open,       all 4 cited names are in the target tree      SHOWN
src/VaultEngine.sol:frob,       (RG20 clean)
src/BalanceSheet.sol:distributeSurplus,
src/PSM.sol:sellStable
restrict vaults on fee-exempt   not run: `check` never applies a patch. Run   NOT SHOWN
stable collateral to the PSM    `realism-gate verify-fix` on the patch and
                                quote its verdict
CLAIMS  COMPLETE  every claim the PoC can show is shown; the fix was not run
```

Read it as the triager would: the attacker row names the test that shows a
plain `makeAddr` sender, the impact row names the revert selectors and the
sibling test that distributes without the setup, the cited row is `RG20`
clean at the pinned commit, and the fix row is open because `check` does not
run patches. Run the same command without `--target`: the `cited` row becomes
`NOT CHECKABLE` and the top line `INCOMPLETE  5 shown, 1 not shown, 1 not
checkable`. Nothing about the PoC changed; the citation predicate had no tree
to read.

The same run with `--card` and `--card-json` writes the
[evidence card](evidence-card.md) that goes with this table.

## What the JSON says

With `--format json`, the report stays `realism-gate.report/v1` and gains two
optional fields. Both are absent when there is no `--report`/`--claims`:

```json
{
  "claims": {
    "schema": "realism-gate.claims/v1",
    "binding": "structured",
    "source": "claims.yaml",
    "outcome": "shown",
    "shown": 6,
    "not_shown": 1,
    "contradicted": 0,
    "not_checkable": 0,
    "rows": [
      {
        "field": "attacker",
        "claim": "any user, no role",
        "evidence": "`alice` (makeAddr(\"alice\")) sends the exploit in test_psmDepositBlocked; no role, no prank as protocol (RG01-RG03 clean)",
        "status": "shown"
      }
    ]
  },
  "claim_binding": "structured"
}
```

`row.field` is one of `attacker`, `impact`, `asset`, `amount`,
`preconditions`, `cited`, `fix`; `row.status` is `shown`, `not_shown`,
`contradicted` or `not_checkable`; `binding` and `claim_binding` are
`structured` or `inferred`; `outcome` is `shown`, `incomplete` or
`contradicted` (the JSON spelling of `COMPLETE`, `INCOMPLETE`,
`CONTRADICTED`). Both are optional properties in the schema
([`docs/output-schema.json`](output-schema.json)), enforced by a test that
validates real structured and inferred output; they are additive within
schema 1, as the compatibility policy requires.

Claims are not SARIF results. SARIF is for rule findings at a file and line;
a claim is a row in a table with its own status vocabulary, and turning a
`NOT SHOWN` into a code-scanning alert would say a line of code is wrong when
the finding's prose is. The rule findings keep their SARIF output unchanged.

## Limits, in one paragraph

Binding reads a report and a PoC, both source. It cannot confirm a role the
test does not grant, an asset the PoC does not name, an amount stated only in
prose, a precondition it has no words for, a citation without a target commit,
or a fix that was never run. A `SHOWN` row means the evidence the predicate
looks for is present; it is not a verdict that the bug is real, that the
severity is right, or that the claim is the one a triager cares about. The
full list is [limitations](limitations.md#claims-and-binding).
