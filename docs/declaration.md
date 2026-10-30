# realism.yaml: declaring the attacker model

The code rules infer who is who from names and data flow. A declaration
replaces that inference with facts the PoC's author states, and adds facts no
code can show: which steps depend on someone else acting, and in which
transaction each step runs.

Write it for the triager, not for the tool. A declaration that makes the output
quiet by declaring something untrue has moved the problem from the PoC into a
file the triager will also read.

## Where it is read from

`realism-gate check` reads `./realism.yaml` if it exists, or the file named by
`--declaration`. `--no-declaration` ignores it. `realism-gate init` writes a
commented template.

One declaration applies to everything checked in that run. For a directory
holding several unrelated PoCs, run once per PoC.

## Schema (version 1)

```yaml
version: 1          # required; this build reads version 1
chain: evm          # the only chain model in v0.1

attacker:
  identities: [attacker, "address(exploit)"]

victim:
  identities: [alice]

privileged:
  - identity: keeper
    role: keeper
    reason: harvest() runs every block from a public keeper network

preconditions:
  - step: a user deposits into the empty vault
    performed_by: victim          # attacker | victim | privileged | third_party | state
    boundary: own_transaction     # own_transaction | victim_transaction | same_block
                                  # | callback_from_victim_call | external_trigger
    ordering_guarantee: ""        # required in practice for attacker + same_block
```

Unknown keys, unknown enum values, a version other than 1 and a privileged
entry with an empty reason are all errors (exit status 2). The schema is
strict on purpose: a typo that silently does nothing is worse than a refusal.

Identities are written as they appear in the test. `address(x)` and
`payable(x)` are the same identity as `x`.

## What each section changes

| Section | Effect |
|---|---|
| `attacker.identities` | These, and only these, are the attacker. The name heuristic is switched off. RG01 now fires on a grant to any of them. |
| `victim.identities` | Treated as ordinary user accounts. |
| `privileged` | RG02 is silent for these identities. The entry is the claim that the exploit may rely on that role acting, and the reason is why. |
| `preconditions` | Read by RG12, RG13 and RG14. |

## The boundary rules

On the EVM a transaction has exactly one sender. That one fact is what RG12
enforces:

- **RG12** (error): `performed_by: attacker` with `boundary: victim_transaction`.
  Nobody can append a call to a transaction another account signs. If the
  victim's own call reaches attacker code (a token hook, a callback), that is
  `callback_from_victim_call`, which is allowed.
- **RG13** (warn): `performed_by: attacker` with `boundary: same_block` and no
  `ordering_guarantee`. Being next to another transaction needs a mechanism.
- **RG14** (note): any precondition performed by a victim, a privileged role or
  a third party. Not a defect; a condition the severity argument must carry.

## Example

`tests/fixtures/declaration/` holds a PoC and its declaration. Without the
declaration, the attacker's unusual name means RG01 cannot see that the
harness grants it a role, and the keeper's harvest is flagged as a privileged
step. With it, RG01 fires, RG02 is silent for the keeper, and the boundary
rules report on the stated preconditions.
