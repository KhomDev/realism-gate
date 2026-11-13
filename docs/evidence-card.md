# The evidence card

An evidence card is what a hunter attaches to a submission, or a triager opens
next to the report: one page that says what the PoC shows claim by claim, and
the hashes a reader needs to check that the files did not change. **It is
signed by nothing; it is reproducible by command.**

```bash
realism-gate check test --report finding.md --card card.md --card-json card.json
```

`--card` writes Markdown for a human; `--card-json` writes the same card as
`realism-gate.card/v1` for a machine. Both need a claim table, so both need
`--report` or `--claims`; without one of them the command exits 2 with
`--card needs --report or --claims: there is no claim table to publish`.

The card does not run anything, sign anything, or contact anything. Every
field is a hash of a file named on the card, or a value the printed rerun
command reproduces.

## What the card states

| Markdown field | JSON key | What it is |
|---|---|---|
| tool | `tool`, `version` | the binary that wrote the card |
| rule set | `rule_set_sha256` | sha256 of every rule's id, slug and severity in catalog order, one per line. Changing a rule's meaning, slug or severity changes the hash; an internal refactor does not. This is how a reader knows which rules produced the table |
| PoC | `poc_sha256` | sha256 over each file the run read: display path, a NUL, the source, a NUL, in load order. A moved or edited file changes the hash. The Markdown shows the first 16 hex characters; the JSON has all 64 |
| report | `report_sha256` | sha256 of the finding's text with CRLF normalised to LF, or `null`/`not given` when `--report` was not passed |
| claims | `claims_sha256`, `claims_source` | sha256 of the structured claims text (the `claims.yaml` file or the front-matter block), and where it came from. Absent when the binding is inferred: there is no text to hash |
| claim binding | `claim_binding` | `structured` or `inferred`; mirrors the table |
| target commit | `target_commit` | the commit of the tree `RG20` and the `cited` predicate read, or `null`/`not pinned` |
| outcome | `claims.outcome` | the table's top line, lower-case in JSON: `shown`, `incomplete`, `contradicted` |
| claim table | `claims` | the same table `--report` prints: schema, binding, source, outcome, four counts, and one row per field with its status |
| Reproduce | `rerun` | the exact `realism-gate check` command that reproduces this table, with paths quoted when they contain a space |
| — | `signed_by` | the statement that nothing is signed: every field is a hash of a file named here, or a value the rerun command reproduces |

The Markdown card abbreviates each hash to its first 16 characters so it
stays readable. **The JSON twin is the authoritative one**: it carries the
full digests, and it is what a script should compare.

## Signed by nothing

There is no key, no timestamp, no signature and no authority behind a card.
Anyone can produce one for any report and any PoC, and the card does not say
otherwise. What the hashes give a reader is narrower and more useful: the
identity of the exact files the table was produced from, and a command that
should print the same table on those files. If the command prints a different
table, the files or the tool changed, and the card says which files and which
tool version to look at.

That is also why the card is not proof of a finding. A card can show that a
PoC's evidence matches a report's claims; it cannot show that the bug is real,
that the severity is right, that the program pays, or that nobody reported it
first. See [limitations](limitations.md#claims-and-binding).

## Golden card: RAIN-USDR

The fixture is [`examples/rain-usdr`](../examples/rain-usdr): the case
study's report, its [`claims.yaml`](../examples/rain-usdr/claims.yaml), a
Foundry test with the four behaviours, and the cited contracts as source. The
cited row needs a git tree, so the fixture ships a script that builds one at a
fixed relative path with a fixed identity and fixed dates; the commit id is
the same on every run.

```bash
cd examples/rain-usdr
sh ./make-target-repo.sh
# TARGET examples/rain-usdr/.target-repo
# COMMIT 5172ee079348b6f56469b148c3808857e1d4d892

realism-gate check test --report finding.md \
  --target .target-repo --target-commit 5172ee079348b6f56469b148c3808857e1d4d892 \
  --card card.md --card-json card.json
```

`card.md`, written by realism-gate 1.0.0:

```markdown
# Evidence card: realism-gate 1.0.0

What this PoC shows, claim by claim, with the hashes a reader needs to check that
nothing changed. **Signed by nothing.** Every field is a hash of a file, or a value
the rerun command reproduces.

| Field | Value |
|---|---|
| tool | `realism-gate 1.0.0` |
| rule set | RG01-RG20, sha256 `ae1794ca39ec7159` |
| PoC | sha256 `646772314298b370` (`test/ShadowVault.t.sol`) |
| report | sha256 `0b5fba2ac3b7fa82` (`finding.md`) |
| claims | sha256 `8b5bc392845c08e8` (claims.yaml) |
| claim binding | structured |
| target commit | `5172ee079348b6f56469b148c3808857e1d4d892` |
| outcome | COMPLETE  every claim the PoC can show is shown; the fix was not run |

## Claim table

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

## Reproduce

```bash
realism-gate check test --report finding.md --target .target-repo --target-commit 5172ee079348b6f56469b148c3808857e1d4d892
```

A reader who runs that command on this tree gets this table. The JSON twin carries
the full digests, and docs/evidence-card.md gives the recipe for each one
(`poc_sha256` hashes every checked file's path and source, not one file).
Nothing here is signed: there is no key, no timestamp and no authority behind
it, only a command.
```

`card.json`, the same card as `realism-gate.card/v1`:

```json
{
  "schema": "realism-gate.card/v1",
  "tool": "realism-gate",
  "version": "1.0.0",
  "rule_set_sha256": "ae1794ca39ec715946fe9623e0218fab9352bf418466dc03a5ed9f2a18657ef3",
  "poc_sha256": "646772314298b3700b4b0577834b6e7df98fe145e39416fd1ec385439df29515",
  "report_sha256": "0b5fba2ac3b7fa8271ac68bcf91f6fa927b3d156b9adba881bc649d4cb3c8f71",
  "claims_sha256": "8b5bc392845c08e8aca58a39f7c8483d9b47f04c538349fbed0af71f2be6d629",
  "claims_source": "claims.yaml",
  "claim_binding": "structured",
  "target_commit": "5172ee079348b6f56469b148c3808857e1d4d892",
  "rerun": "realism-gate check test --report finding.md --target .target-repo --target-commit 5172ee079348b6f56469b148c3808857e1d4d892",
  "signed_by": "nothing: every field is a hash of a file named here, or a value the rerun command reproduces",
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
      },
      {
        "field": "impact",
        "claim": "other:distributeSurplus reverts with ReserveBackingShortfall (revenue halted) and PSM.sellStable reverts with CeilingExceeded (on-ramp denial)",
        "evidence": "vm.expectRevert(BalanceSheet.ReserveBackingShortfall.selector) in test_shadowDrawBlocksDistribution; vm.expectRevert(PegStabilityModule.CeilingExceeded.selector) in test_psmDepositBlocked; negative control test_healthyDistribution_shipsSurplus acts without the setup and does not revert",
        "status": "shown"
      },
      {
        "field": "asset",
        "claim": "USDR",
        "evidence": "`assertEq(drawn, locked)` in test_shadowDrawBlocksDistribution compares two quantities of `USDR`",
        "status": "shown"
      },
      {
        "field": "amount",
        "claim": "the drawn USDR equals the locked stablecoins, so the position is capital-neutral",
        "evidence": "`assertEq(drawn, locked)` in test_shadowDrawBlocksDistribution compares the two quantities the claim is about",
        "status": "shown"
      },
      {
        "field": "preconditions",
        "claim": "no privileged role; no oracle or price movement; no keeper and no off-chain trigger",
        "evidence": "`no privileged role`: RG01-RG03 clean; `no oracle or price movement`: no fork, RPC or chain read in the PoC; `no keeper and no off-chain trigger`: every exploit step is the same sender's own call",
        "status": "shown"
      },
      {
        "field": "cited",
        "claim": "src/VaultEngine.sol:open, src/VaultEngine.sol:frob, src/BalanceSheet.sol:distributeSurplus, src/PSM.sol:sellStable",
        "evidence": "all 4 cited names are in the target tree (RG20 clean)",
        "status": "shown"
      },
      {
        "field": "fix",
        "claim": "restrict vaults on fee-exempt stable collateral to the PSM",
        "evidence": "not run: `check` never applies a patch. Run `realism-gate verify-fix` on the patch and quote its verdict",
        "status": "not_shown"
      }
    ]
  }
}
```

The two files are committed next to the fixture, at
[`examples/rain-usdr/card.md`](../examples/rain-usdr/card.md) and
[`examples/rain-usdr/card.json`](../examples/rain-usdr/card.json), and
`tests/docs.rs` regenerates them with `REALISM_GATE_BLESS=1 cargo test --test docs`.
The card is only as current as its files. The golden card above is a
snapshot; regenerating it on a different checkout is expected to change
`poc_sha256` if any test source changed, and `report_sha256`/`claims_sha256`
if the report did. `rule_set_sha256` changes only when the rule catalog does.

## Checking the hashes yourself

```bash
cd examples/rain-usdr

# report and claims: sha256 of the text, with CRLF normalised to LF
sha256sum finding.md claims.yaml
# 0b5fba2ac3b7fa8271ac68bcf91f6fa927b3d156b9adba881bc649d4cb3c8f71  finding.md
# 8b5bc392845c08e8aca58a39f7c8483d9b47f04c538349fbed0af71f2be6d629  claims.yaml
```

The PoC hash is not `sha256sum` of one file. It covers every file the run
read, as display path + NUL + source + NUL, in load order, which is why it
cannot change when a file moves or its bytes change. For this one-file
fixture:

```bash
python3 - <<'PY'
import hashlib
h = hashlib.sha256()
for path in ["test/ShadowVault.t.sol"]:
    h.update(path.encode() + b"\0")
    h.update(open(path, "rb").read() + b"\0")
print(h.hexdigest())
PY
# 646772314298b3700b4b0577834b6e7df98fe145e39416fd1ec385439df29515
```

(On a Windows checkout with CRLF line endings, read the file in text mode and
replace `\r\n` with `\n` first; the reader normalises line endings before
hashing. The same applies to `sha256sum` for the report and claims.)

If the hashes match and the rerun command prints the same table, the card is
accurate about those files. That is the whole claim it makes.
