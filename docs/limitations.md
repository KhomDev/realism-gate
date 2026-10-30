# What realism-gate cannot tell you

A tool that checks proofs should be exact about what it does not check.

## It reads the test, not the protocol

- It does not know the deployed contract's access control. "This address is
  privileged" is inferred from names, labels, getters and grants in the test.
  A role it cannot see is a role it cannot flag, and a name that merely sounds
  privileged will be flagged (`docs/rules/RG02.md` lists the known cases).
- It does not know whether the configuration a test sets up matches the real
  deployment. RG02 flags a privileged step in the exploit; a privileged step in
  `setUp()` is treated as configuration and not reported at all.
- It does not know whether a mocked or etched dependency behaves differently
  from the real one. RG05 and RG07 report that the PoC stopped testing the real
  thing, not that the result is wrong.

## It reads Solidity, not the toolchain

- Fork settings in `foundry.toml` or on the command line are invisible, so a
  fork pinned there still trips RG11. Use an `allow` comment.
- Code in other contracts the test calls externally is not followed. An
  assertion inside an external helper contract does not count for RG08.
- `invariant_*` and stateful fuzz entry points are not checked in v0.1.
- Yul blocks are skipped.

## It approximates

- Control flow is flattened. A prank inside an `if` counts whether or not the
  branch runs.
- Data flow is followed through plain assignments and a few levels of aliasing,
  not through storage, return values of external calls, or arrays.
- Solidity's C3 linearization is approximated depth first. For ordinary test
  hierarchies the most derived definition is the same.

## A clean result is not a valid PoC

realism-gate finds known ways a PoC proves less than it claims. It cannot tell
you that the bug is real, that the impact is what the report says, that the
program pays for it, or that nobody reported it first. A PoC with zero findings
can still be wrong, and a PoC with findings can still be right once the author
has justified each one.

For the other half of that question (whether a written finding survives
triage), see [viability-gate](https://github.com/KhomDev/viability-gate).
