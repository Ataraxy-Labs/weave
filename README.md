# Weave

Entity-level patches and merges for Git.

Weave parses code into functions, classes, and other entities to merge independent changes that line-based merging may conflict on. Keep using Git; Weave runs underneath it. [Sem](https://github.com/Ataraxy-Labs/sem) helps you find and understand code.

## Install

```bash
brew install weave
```

[Other installation options](USAGE.md#install)

## Quickstart

```bash
weave setup                 # Configure this repository
git merge <branch>          # Merge through Git as usual
weave explain <file>        # Understand a remaining conflict
# Edit the file to resolve it
weave check                 # Check the resolution for merge problems
```

Use `weave setup --global` for all repositories, or `weave setup --off` to remove the integration from this repository.

## Commands

| Command | Purpose |
|---|---|
| `setup` | Configure the Git merge driver |
| `preview` | Inspect a potential merge without writing it |
| `explain` | Understand conflicts and merge markers |
| `check` | Check merge integrity |
| `patch extract` / `patch apply` | Create and apply entity patches, including against drifted source |
| `land` | Resolve and gate a merge; `--onto` also integrates and publishes |
| `stats` | Inspect counters or run merge benchmarks |
| `experimental claim` / `release` | Coordinate edits with advisory claims |

Run `weave <command> --help` for options. [Full reference](USAGE.md#cli-commands)

## Use with agents

For Claude Code:

```bash
claude mcp add --scope user weave -- weave-mcp
```

Other MCP clients can launch `weave-mcp` over stdio. Set its working directory or `WEAVE_REPO` to your repository.

The server provides merge analysis, entity inspection, and optional live coordination. Claims are not required for ordinary Git merges. [Agent setup and tools](USAGE.md#mcp-server)

## Checks and safety

- `weave check` checks merge integrity; `sem check` runs project checkers.
- Structural checks do not prove program behavior. Run your build and tests.
- Claims are experimental and advisory; they do not block another editor.
- `weave land --onto` publishes changes. Use it only when you intend to publish.
- Unsupported files fall back to line-level merging. Some parsed languages are deliberately excluded from structural merging.

## Learn more

[Languages](USAGE.md#supported-languages) · [Landing](USAGE.md#landing-an-agents-merge-weave-land) · [Benchmarks](USAGE.md#real-world-benchmarks) · [Jujutsu](USAGE.md#jujutsu-jj) · [Architecture](USAGE.md#architecture) · [Contributing](USAGE.md#contributing) · [Releases](https://github.com/Ataraxy-Labs/weave/releases)

Part of [Ataraxy Labs](https://ataraxy-labs.com). See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
