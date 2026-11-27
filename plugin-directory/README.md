# realism-gate — Anthropic Directory plugin

A tighter Claude Code plugin for
[realism-gate](https://crates.io/crates/realism-gate). It ships the skill and
read-only commands. It does **not** install or run the CLI: you run
`cargo binstall realism-gate` (or `cargo install --locked realism-gate`) and
the checker yourself, then paste or save the output.

This is the bundle submitted to the Anthropic plugin directory. The Directory
does not accept hooks, a `bin/` installer, or shell `allowed-tools` from an
unverified publisher. The full plugin, which can install and run the pinned
CLI, stays in [`plugin/`](../plugin/) and is unchanged.

## Install

From this repository, in Claude Code, add the marketplace and install the
plugin, or load the `plugin-directory/` path the Directory listing names.

Then `/realism-gate:install` (shows the cargo command; it does not run it)
and `/realism-gate:check test/ --fail-on warn` (shows the check command; you
run it).

## What is in this tree

- `skills/realism-gate/SKILL.md` — the repository skill, worded so the agent
  asks you to run the CLI or MCP tools instead of running them itself.
- `commands/check.md`, `install.md`, `upgrade.md` — the same three commands
  as the full plugin, with `allowed-tools` limited to `Read`, `Grep`, `Glob`
  and `LS`.
- `.claude-plugin/plugin.json` and `.claude-plugin/icon.png` (1024×1024).

There is no `hooks/` directory, no `bin/` directory, and no script. MCP setup
is still yours: after the CLI is on `PATH`, `claude mcp add rgate -- realism-gate-mcp`,
as in [`docs/agents.md`](../docs/agents.md).

`tests/plugin_directory.rs` fails if a hook, a `bin/` path, a disallowed
tool, a `$` in `allowed-tools`, or a missing/undersized icon is added.
