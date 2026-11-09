# Using realism-gate from an agent

Two pieces, both optional and both local: a skill the agent reads, and an MCP
server that runs the checkers.

## Skill

The skill is [`skills/realism-gate/SKILL.md`](../skills/realism-gate/SKILL.md).
It tells the agent to gate a PoC and a draft before writing a report.

Claude Code reads a skill from the project's `.claude/skills/` or from
`~/.claude/skills/`. Codex reads one from `~/.codex/skills/` or from a project
skill directory your Codex build is configured to load.

```bash
# Claude Code, this repo
mkdir -p .claude/skills/realism-gate
cp skills/realism-gate/SKILL.md .claude/skills/realism-gate/SKILL.md

# Claude Code, every project
mkdir -p ~/.claude/skills/realism-gate
cp skills/realism-gate/SKILL.md ~/.claude/skills/realism-gate/SKILL.md

# Codex
mkdir -p ~/.codex/skills/realism-gate
cp skills/realism-gate/SKILL.md ~/.codex/skills/realism-gate/SKILL.md
```

Copying the file into the tool's skill directory is the install. Nothing is
downloaded.

## MCP server

`realism-gate-mcp` speaks MCP over stdio. It has two tools:

| Tool | Runs | Stops the agent when |
|---|---|---|
| `check_poc` | `realism-gate check --format json` | the exit code is non-zero (default `--fail-on warn`) |
| `check_finding` | `vg check --json`, and `realism-gate` when a PoC path is passed | viability-gate says `KILL` or `HOLD`, or the PoC check is non-zero |

It shells out. It does not reimplement either checker. A `path` that starts
with `-` is rejected, and paths are passed after `--`, so the argument cannot
inject `--skip`. The server passes `--no-config` and does not load a
`realism.yaml` from the working tree. Tool output is prefixed as data, not as
instructions.

```bash
cargo install --path . --locked
# binaries: realism-gate, rgate, realism-gate-mcp
```

`REALISM_GATE_BIN` and `VG_BIN` override the binaries. Otherwise the server
looks next to its own executable, then on `PATH`. `vg` is
[viability-gate](https://github.com/KhomDev/viability-gate).

Claude Code (`~/.claude.json` or the project's `.mcp.json`):

```json
{
  "mcpServers": {
    "realism-gate": {
      "command": "realism-gate-mcp"
    }
  }
}
```

Codex (`~/.codex/config.toml`):

```toml
[mcp_servers.realism-gate]
command = "realism-gate-mcp"
```

The server reads Content-Length frames on stdin and writes them on stdout.
A notification gets no response. `initialize`, `tools/list`, `tools/call` and
`ping` are the methods it handles.
