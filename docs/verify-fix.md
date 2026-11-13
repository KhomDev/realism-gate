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
`..` is refused. `git apply` is run without `--unsafe-paths`. If git cannot
apply the diff, a small built-in applier writes only paths that already passed
that check. The tree you started with is not modified.

## Do not run an untrusted project on the host

A Foundry project can set `ffi = true` and can name an RPC URL. `verify-fix`
refuses to start when `foundry.toml` sets `ffi = true`, and it sets
`FOUNDRY_FFI=false` on the forge process. That is not a sandbox.

On a machine that is not a container, the command exits with an error until
you pass one of:

- `--sandbox`, which means you attest that this process is confined: a
  container or similar, `ffi` off, and network only to the pinned RPC the PoC
  needs.
- `--allow-host`, which runs forge here anyway. `ffi` is still refused.

A container is detected when `/.dockerenv` or `/run/.containerenv` exists, or
when `REALISM_GATE_SANDBOX=1`. That detection is not a guarantee. If you are
gating a PoC you do not trust, run the command in a container you built for
that, with no extra network, and pass `--sandbox`.

`check` never calls this command and never runs forge.
