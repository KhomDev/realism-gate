# verify-fix

`realism-gate verify-fix` is the only command that runs a test. It answers one
question: did this PoC pass on the code before the patch, and fail on the code
after it?

It is not a validity oracle. A pass means the named test went from passing to
failing under forge. It does not mean the patch is correct, complete, or the
only way to fix the bug.

## Flags

| Flag | Default | Meaning |
|---|---|---|
| `--poc FILE` | required | the Foundry test to run |
| `--patch FILE` | — | a unified diff applied to a copy of the project before forge runs |
| `--patched DIR` | — | a directory that already holds the patched sources; conflicts with `--patch` |
| `--match-test REGEX` | — | passed to `forge test --match-test` |
| `--forge PATH` | `forge` on `PATH` | the forge binary to run; name the full path when a wrapper or a version manager hides it |
| `--root DIR` | the current directory | the project to copy when `--patch` is used. With `--patched`, the unpatched side is the project in the current directory |
| `--sandbox` | off | attest that this process is confined (container, `ffi` off, network only to the pinned RPC); conflicts with `--allow-host` |
| `--allow-host` | off | run forge on this host anyway; `ffi = true` is still refused |

`--forge` and `--root` change which binary and which tree are used; they do not
change the verdict rule. The same two runs, pass before the patch and fail
after it, decide the result.
## Both sides

1. Copy the project and run `forge test --json` on that copy. The PoC must
   pass. If it fails, the command reports that the bug did not reproduce. A
   later failure on the patched tree is not a fix.
2. Apply the patch to a second copy, or copy `--patched`, and run the same
   test. The PoC must fail.

The verdict is the JSON document forge writes to stdout, plus the process exit
code. Those two have to agree. A log line, including one that contains the
text `"success": false`, is not read as a result.

A compile error, or JSON with no test result, is a failure to run the PoC.
It is not a failed PoC and it is not a fix.

## The patch stays in the temp tree

The diff is checked before it is applied. A path that is absolute or contains
`..` is refused. `git apply` is run without `--unsafe-paths`, and `git` is
required: `git apply --check` runs first, then the apply. There is no
hand-written fallback applier, because one that accepted a different subset of
diffs could leave a tree the patch does not describe, and a verdict on that
tree would be meaningless. If git is missing or the patch does not apply
cleanly, the command exits with an error and forge does not run. The tree you
started with is not modified.

Three more limits apply. A patch that creates or changes a symbolic link (mode
`120000`) is refused, because `git apply` would create a link whose target is
not checked. A symbolic link in the project is not copied, and a patch aimed at
it writes only inside the copy; `src/verify_fix.rs` has the test. A patch larger
than 8 MiB is refused before it is read. Windows drive prefixes, the verbatim
prefix (two backslashes, a question mark, a backslash) and UNC paths are refused
like absolute paths.

## Do not run an untrusted project on the host

A Foundry project can set `ffi = true` and can name an RPC URL. `verify-fix`
refuses to run when any `foundry.toml` forge could read enables `ffi`. The
check is a TOML parse, not a text heuristic, so spacing, tabs, quoted keys,
dotted keys, inline tables and profiles it has never heard of cannot change the
answer. Three trees are checked, and every check happens before a forge
process is started:

1. the root project (`--root`, or the current directory);
2. the `--patched` tree;
3. the copy the `--patch` is applied to, after the apply — a patch that adds
   `ffi = true` is refused.

Any key named `ffi` with the boolean value `true`, anywhere in the parsed
document, is a refusal. That is deliberately conservative: a profile this
program does not know about is still covered.

`FOUNDRY_FFI=false` is set on the forge process, but it is not a control.
Foundry documents `FOUNDRY_FFI` as an explicit exception to its environment
overrides, so this program does not rely on the value: the TOML parse above is
what stops an ffi project, and the container boundary is what contains the
rest.

On a machine that is not a container, the command exits with an error until
you pass one of:

- `--sandbox`, which means you attest that this process is confined: a
  container or similar, `ffi` off, and network only to the pinned RPC the PoC
  needs.
- `--allow-host`, which runs forge here anyway. `ffi` is still refused.

The host decision is enforced inside the library entry point (`verify`), not
only by the CLI: a caller that supplies no policy is refused before any tree is
copied or any process is started.

A container is detected when `/.dockerenv` or `/run/.containerenv` exists, or
when `REALISM_GATE_SANDBOX=1`. That detection is not a guarantee. If you are
gating a PoC you do not trust, run the command in a container you built for
that, with no extra network, and pass `--sandbox`.

The detection trusts the environment and the marker files. Anything that can
set `REALISM_GATE_SANDBOX=1` or create `/.dockerenv` passes it, and an agent
with a shell can do both. The command is not offered through the MCP server.
The current policy is pinned by `tests/verify_fix_limits.rs`; changing it is a
decision for the maintainer, not a side effect of a patch.

`check` never calls this command and never runs forge.
