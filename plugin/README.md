# realism-gate — Claude Code plugin

Checks exploit PoC tests and a draft finding's claims with
[realism-gate](https://crates.io/crates/realism-gate) from inside Claude Code.

## Install

```
/plugin marketplace add KhomDev/realism-gate
/plugin install realism-gate@realism-gate
```

Then `/realism-gate:install` (installs the CLI from crates.io through cargo) and
`/realism-gate:check test/ --fail-on warn`.

## Linked to the crate

The plugin's version **is** the realism-gate crates.io version it drives
(`.claude-plugin/plugin.json`). `bin/realism-gate-ensure` installs exactly that
version (`cargo binstall` when present, else `cargo install --locked`), and
`tests/plugin.rs` in the repository fails if the plugin, the marketplace entry
and `Cargo.toml` disagree, or if the plugin's skill differs from
`skills/realism-gate/SKILL.md`. Set `REALISM_GATE_BIN` to use a binary you
built yourself.

## Upgrade

- `/plugin marketplace update realism-gate` then `/plugin update realism-gate`:
  the new plugin pins the matching CLI; the next check installs it.
- `/realism-gate:upgrade` explains both paths and can move the CLI alone to the
  newest crates.io release.

The SessionStart hook only reports a missing or mismatched CLI; it never
installs anything. The CLI itself is offline; only the install step uses the
network, through cargo.

## Commands

| Command | What it does |
|---|---|
| `/realism-gate:check [paths] [--report finding.md]` | runs `realism-gate check` and explains the findings |
| `/realism-gate:install` | installs or repairs the pinned CLI |
| `/realism-gate:upgrade` | plugin or CLI upgrade |
