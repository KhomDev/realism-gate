# Labelling rubric for external labellers (rubric v3)

This page is all a labeller needs. It restates rubric v3
(`eval/P2R_PREREGISTRATION.md` §4 and the eight base decisions of
`eval/solana-poc-v2/PREREGISTRATION.md`) in one place. It adds no new rule. Where
the wording differs, the pre-registration governs.

## What you are judging

Each item is one **flag**. A flag is one finding that `realism-gate check` raised on one test file.
You get the flag's rule ID, its message (the claim), the rule page (`docs/rules/RGxx.md`), and
the source file with the flagged lines. You **do not** get the severity the tool raised or
shipped, or anyone else's label, and you must not try to find them.

Answer one question: **is the flag's claim true of this test?** Use one of:

| Label | Meaning |
|---|---|
| `true-positive` | The claim in the message holds for this test, as the rule page defines it. |
| `false-positive` | The claim does not hold. Give the reason. |
| `unclear` | The file alone cannot decide it, for example because the claim depends on code that is not in the unit. Give the reason. |

Every label has a reason of one or two sentences that names the lines and, where one applies, the
decision below. `unclear` is not a way to skip hard items. A rule cell with more than 20% `unclear`
is reported as inconclusive.

## Decisions that apply to every rule

1. **Judge the claim, not the exploit.** A platform's `invalid` or `valid` verdict on the
   submission does not make a flag true or false.
2. **Unreadable units are not labelled.** They have no flags. You will not see them.
3. **Program source outranks the test harness.** If the program and the test disagree about
   authority (for example, a payer marked as a role signer in the test but not required by the
   program), follow the program source at the pinned commit and cite the program lines.
4. **Control tests** (they expect failure, or their title says secure/control/prevents/rejects/
   "fails when"): RG01, RG02, RG03, RG04 and RG06 are `false-positive` if they fire. RG08 and RG09
   are judged on their own defect.

## Rule-specific decisions

5. **RG08 (no assertion on the outcome)** covers the exploit part of the trace. An assertion in setup
   does not silence it, and a test that asserts nothing at all is `true-positive`.
6. **A precondition fabricated in setup** (RG04/RG06 on a victim's funded account, with the drain
   going through the program) is `true-positive` for the finding whose message begins
   `Precondition fabricated:`. That finding claims only that the precondition never passed
   through the program's accounting.
7. **A fixture the program initialises** (the test writes an account, then an init/create
   instruction of the program under test sets it up) is not a finding: `false-positive`.
8. **The attacker's own key** (a fresh key with no grant, role field, initialise call or
   declaration) is the attacker. RG01, RG02 and RG03 are `false-positive` if they call it privileged.
9. **The attacker taking the role through the bug** (it signs the grant or re-initialisation
   itself) is the exploit, not a harness grant. RG01 is `false-positive` on that grant.
10. **RG10 (no baseline):** a read of the same account before the attack is a baseline. RG10 is
    `false-positive` if it fires when such a read exists.
11. **Solidity, by analogy:** the unit is a Foundry test, and `deal`, `prank` and `store` are the
    mutations.

## Protocol

- Two labellers, A and B, work independently. B is blind to A. Neither sees the severity.
- Agreement is raw agreement and Cohen's kappa over the two labels before adjudication. A cell with kappa
  below 0.70 is inconclusive.
- A third, written adjudication settles every disagreement, citing a decision above. The
  adjudicated label is the one counted.
- Labeller identities, the rubric version and the date go into the round file before labelling
  starts. A rubric change after labels exist is a new version and never re-labels an opened round.

## Practice pack

`eval/labeling-pack/` has a ~50-flag pack drawn from **DEV (opened) data only**, for calibration
and for timing. Its labels never enter a held-out cell.