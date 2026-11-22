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

## Bounded runs

The copy is attacker-controlled, so it is capped before and while it is made:
50 000 files, 512 MiB in total, 50 000 directories, and 64 levels deep. A tree
over any of those is refused with an error naming the limit, before forge
starts.

Each forge run has a wall-clock limit of 10 minutes and stream caps of 64 MiB
on stdout and 1 MiB on stderr. A run that exceeds the time or either stream
cap is killed and is an error (exit 2), never a "fail": a verdict is not taken
from a run that did not finish or from a truncated log. The direct child is
killed. A Foundry config (a tree's or the global one) is read only when it is
a regular file of at most 1 MiB; a larger one is refused before the read.

Not bounded: the disk space forge itself uses (the copy is capped before it
starts), a network connection the test itself opens, and a descendant process
that outlives the killed forge. Those are the container's job.

## The patch stays in the temp tree

The diff is checked before it is applied. A path that is absolute or contains
`..` is refused. The patch path is resolved from the directory you run the
command in before `git apply` runs with the temp copy as its working
directory, so `--patch ../fix.diff` means the file next to the project, not a
file under the copy; a patch inside the project also works. The same rule
applies to a `--forge` value that contains a path separator; a bare
`--forge forge` keeps PATH lookup. `git apply` is run without
`--unsafe-paths`, and `git` is required: `git apply --check` runs first, then
the apply. There is no hand-written fallback applier, because one that
accepted a different subset of diffs could leave a tree the patch does not
describe, and a verdict on that tree would be meaningless. If git is missing
or the patch does not apply cleanly, the command exits with an error and
forge does not run. The tree you started with is not modified.

Three more limits apply. A patch that creates or changes a symbolic link (mode
`120000`) is refused, because `git apply` would create a link whose target is
not checked. A symbolic link in the project is not copied, and a patch aimed at
it writes only inside the copy; `src/verify_fix.rs` has the test. A patch larger
than 8 MiB is refused before it is read. Windows drive prefixes, the verbatim
prefix (two backslashes, a question mark, a backslash) and UNC paths are refused
like absolute paths.

## Do not run an untrusted project on the host

A Foundry project can set `ffi = true` and can name an RPC URL. `verify-fix`
refuses to run when any Foundry config it can check enables `ffi`. The check
is a TOML parse, not a text heuristic, so spacing, tabs, quoted keys, dotted
keys, inline tables and profiles it has never heard of cannot change the
answer. Four configs are checked:

1. the global config, `~/.foundry/foundry.toml` (`%USERPROFILE%\.foundry\foundry.toml`
   on Windows, with `HOME` as the fallback);
2. the root project (`--root`, or the current directory);
3. the `--patched` tree;
4. the copy the `--patch` is applied to, after the apply — a patch that adds
   `ffi = true` is refused.

The global config and the root are parsed before anything is copied, the
`--patched` tree before it is copied, and the post-patch copy after the apply.
The global config and both copies are parsed again immediately before **each**
forge start (so also before the second run).

Any key named `ffi` with the boolean value `true`, anywhere in a parsed
document, is a refusal. That is deliberately conservative: a profile this
program does not know about is still covered. A checked config that does not
parse as TOML is refused too, rather than assumed off.

The same parse enforces `fs_permissions`: every entry must grant exactly
`access = "read"`. An entry with `access = "write"` or `access = "read-write"`,
an entry with no `access`, or an `fs_permissions` value that is not an array is
refused before any forge run. This is the primary control against a cross-run
bypass: with write access the first PoC run could rewrite the other copy's (or
the global) config to `ffi = true`, and the second run would then execute ffi.
The re-parse before the first and the second run is defense in depth for the
simple rewrite; a lingering process that writes after a check is covered only
by the container boundary.

A project that grants itself write access is refused even when the PoC does
not use it; narrow the entry to `access = "read"` for `verify-fix`.

The global config path is resolved from `USERPROFILE` (then `HOME`) on
Windows and from `HOME` elsewhere. Foundry may resolve the OS known profile
folder instead, so a config in that folder can differ from the one checked
when those variables are overridden; `docs/threat-model.md` H5 records that
residual.

`FOUNDRY_CONFIG` is set on the forge child to the absolute path of a config
inside the copy that this command checked. If the tree has no `foundry.toml`,
a minimal `[profile.default]` file is created in the copy and pinned. This is
deliberate: Foundry loads a `.env` after it starts, and dotenvy does not
override an already-set variable, so deleting `FOUNDRY_CONFIG` would let a
`.env` point it at a config the guard never parsed. As defense in depth, a
`.env` in any copied tree is refused before each forge run; `copy_tree` skips
dotfiles, so the vector is a `--patch` that creates one.

**Residual, not covered.** Foundry searches upward from the project directory
for configuration. The copy forge runs in lives under the system temp
directory, so a `foundry.toml` in an *ancestor* of that copy (the temp root,
or a directory above it) is outside this check. So is a config Foundry
reaches through that own search rather than the pinned `FOUNDRY_CONFIG`, the
global file, or a checked tree. Run the command in a container if those are in
your threat model; `docs/threat-model.md` H5 records this as unverified.

`FOUNDRY_FFI=false` is set on the forge process, but it is not a control.
Foundry documents `FOUNDRY_FFI` as an explicit exception to its environment
overrides, so this program does not rely on the value: the TOML parses above
are what stop an ffi project, and the container boundary is what contains the
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
