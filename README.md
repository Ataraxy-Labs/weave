> **Part of the [Ataraxy Labs](https://ataraxy-labs.com) stack** — agent-native infrastructure for software development. See also: [sem](https://ataraxy-labs.com/sem) (semantic version control) · [inspect](https://github.com/Ataraxy-Labs/inspect) (semantic code review) · [opensessions](https://github.com/Ataraxy-Labs/opensessions) (tmux sidebar for coding agents).
>
> Read the manifesto: https://ataraxy-labs.com/#thesis · Essays: https://ataraxy-labs.com/blogs · LLMs: https://ataraxy-labs.com/llms.txt

<p align="center">
  <img src="assets/banner.svg" alt="weave" width="600" />
</p>

<p align="center">
  <strong>Entity-level semantic merge driver for Git.</strong><br>
  Resolves merge conflicts that Git can't by understanding code structure via tree-sitter.
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#quickstart">Quickstart</a> ·
  <a href="#how-weave-fixes-this">How It Works</a> ·
  <a href="#mcp-server">MCP Server</a> ·
  <a href="https://github.com/Ataraxy-Labs/weave/releases/latest">Releases</a>
</p>

<p align="center">
  <a href="https://github.com/Ataraxy-Labs/weave/releases/latest"><img src="https://img.shields.io/github/v/release/Ataraxy-Labs/weave?color=blue&label=release" alt="Release"></a>
  <a href="https://formulae.brew.sh/formula/weave"><img src="https://img.shields.io/badge/homebrew-weave-orange" alt="Homebrew"></a>
  <img src="https://img.shields.io/badge/rust-stable-orange" alt="Rust">
  <img src="https://img.shields.io/badge/tests-401_passing-brightgreen" alt="Tests">
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT_OR_Apache--2.0-yellow" alt="License"></a>
  <img src="https://img.shields.io/badge/languages-38-blue" alt="Languages">
</p>

<p align="center">
  <img src="assets/merge-animation.gif" alt="Weave merge animation: two branches add different functions, git conflicts, weave merges cleanly" width="700" />
</p>

## Quickstart

```bash
weave setup                 # this repo now merges through weave; git merge/rebase/cherry-pick unchanged
git merge <branch>          # real conflicts land as markers with a `refused_by:` line stating why
weave explain <file>        # per-hunk detail for one conflicted file, read off the actual git stages
#  ...edit to resolve...
weave check                 # verify the working tree against the three merge stages; exits 1 on findings
```

See [Setup](#setup) for `--global`/`--local` variants and [MCP Server](#mcp-server) for agent-framework integration.

## The Problem

Git merges by comparing **lines**. When two branches both add code to the same file — even to completely different functions — Git sees overlapping line ranges and declares a conflict:

```
<<<<<<< HEAD
export function validateToken(token: string): boolean {
    return token.length > 0 && token.startsWith("sk-");
}
=======
export function formatDate(date: Date): string {
    return date.toISOString().split('T')[0];
}
>>>>>>> feature-branch
```

These are **completely independent changes**. There's no real conflict. But someone has to manually resolve it anyway.

This happens constantly when multiple AI agents work on the same codebase. Agent A adds a function, Agent B adds a different function to the same file, and Git halts everything for a human to intervene.

## How Weave Fixes This

Weave replaces Git's line-based merge with **entity-level merge**. Instead of diffing lines, it:

1. Parses all three versions (base, ours, theirs) into semantic entities — functions, classes, JSON keys, etc. — using [tree-sitter](https://tree-sitter.github.io/)
2. Matches entities across versions by identity (name + type + scope)
3. Merges at the entity level:
   - **Different entities changed** → auto-resolved, no conflict
   - **Same entity changed by both** → attempts intra-entity merge, conflicts only if truly incompatible
   - **One side modifies, other deletes** → flags a meaningful conflict

The same scenario above? Weave merges it cleanly with zero conflicts — both functions end up in the output.

## Weave vs Git Merge

| Scenario | Git (line-based) | Weave (entity-level) |
|----------|-----------------|---------------------|
| Two agents add different functions to same file | **CONFLICT** | Auto-resolved |
| Agent A modifies `foo()`, Agent B adds `bar()` | **CONFLICT** (adjacent lines) | Auto-resolved |
| Both agents modify the same function differently | CONFLICT | CONFLICT (with entity-level context) |
| One agent modifies, other deletes same function | CONFLICT (cryptic diff) | CONFLICT: `function 'validateToken' (modified in ours, deleted in theirs)` |
| Both agents add identical function | **CONFLICT** | Auto-resolved (identical content detected) |
| Both agents add different properties to same object | **CONFLICT** | Auto-resolved |
| Different JSON keys modified | **CONFLICT** | Auto-resolved |

The key difference: Git produces false conflicts on **independent changes** because they happen to be in the same file. Weave only conflicts on **actual semantic collisions** when two branches change the same entity incompatibly.

## Weave vs Mergiraf

Tested on 31 real-world merge scenarios across Python, TypeScript, Rust, Go, Java, and C:

| Tool | Clean Merges | Score |
|------|-------------|-------|
| **Weave** | **31/31** | 100% |
| Mergiraf (v0.16.3) | 26/31 | 83% |
| Git | 15/31 | 48% |

Mergiraf fails on both-add-at-end-of-file, insert-between-existing, and decorator conflict scenarios. Weave resolves all of these because it operates at entity granularity (functions, classes, methods) rather than AST node level. Full breakdown at [ataraxy-labs.github.io/weave](https://ataraxy-labs.github.io/weave/).

## Real-World Benchmarks

Tested on real merge commits from major open-source repositories. For each merge commit, we replay the merge with both Git and Weave, then compare against the human-authored result.

- **Wins**: Merge commits where Git conflicted but Weave resolved cleanly
- **Regressions**: Cases where Weave introduced errors (0 across all repos)
- **Human Match**: How often Weave's output exactly matches what the human wrote
- **Resolution Rate**: Percentage of all merge commits Weave resolved vs total attempted

| Repository | Language | Merge Commits | Wins | Regressions | Human Match | Resolution |
|------------|----------|---------------|------|-------------|-------------|------------|
| [git/git](https://github.com/git/git) | C | 1319 | 39 | 0 | 64% | 13% |
| [Flask](https://github.com/pallets/flask) | Python | 56 | 14 | 0 | 57% | 54% |
| [CPython](https://github.com/python/cpython) | C/Python | 256 | 7 | 0 | 29% | 13% |
| [Go](https://github.com/golang/go) | Go | 1247 | 19 | 0 | 58% | 28% |
| [TypeScript](https://github.com/microsoft/TypeScript) | TypeScript | 2000 | 65 | 0 | 6% | 23% |

Zero regressions across all repositories. Every "win" is a place where a developer had to manually resolve a false conflict that Weave handles automatically.

## Testing

weave's correctness is checked two ways. The open test suite in this
repository covers the documented merge properties and runs in CI. In
addition, every release is gated by a private conformance suite — currently
2,800+ enumerated merge-rule cells and five corpora of real-world merges —
maintained separately, following the held-out-benchmark practice used by
conformance and evaluation suites elsewhere (SQLite/TH3, Khronos CTS, LLM
eval sets). PRs receive a pass/fail status from this suite automatically.

## Conflict Markers

When a real conflict occurs, weave gives you context that Git doesn't: which
entity, what type, and — on the line inside the box — which internal guard
declined to auto-merge and exactly which lines both sides disagree about.

```
<<<<<<< ours — function `process` (T, confidence: high)
// refused_by: statement_fold · collision: `    return data.upper()` +1 more
export function process(data: any) {
    return JSON.stringify(data);
}
=======
export function process(data: any) {
    return data.toUpperCase();
}
>>>>>>> theirs — function `process` (T, confidence: high)
```

Run `weave explain <file>` for more detail on every conflicted entity in the
file, and `weave check` after editing to verify your resolution against the
three merge stages — see [Quickstart](#quickstart).

## Supported Languages

TypeScript, TSX, JavaScript, Python, Go, Rust, Java, C, C++, Ruby, C#, PHP, Swift, Kotlin, Scala, Dart, Elixir, Bash, Fish, Fortran, Perl, OCaml, Zig, Elm, Clojure, EDN, D, Lua, Nix, SQL, HCL/Terraform, LaTeX, XML, JSON, YAML, TOML, CSV, Markdown. Falls back to standard line-level merge for everything else.

`weave setup` writes a `merge=weave` line for exactly this list and nothing
else. Each language on it passes a five-scenario merge sweep — two sides adding
different definitions merges clean, two sides rewriting the same definition
conflicts, and nothing is dropped — in `crates/weave-core/tests/language_coverage.rs`.

Vue, Svelte, ERB and Haskell are parsed but deliberately **not** claimed. Their
entity model treats a whole `<script>` block, template, or type signature as a
single unit, so two people adding two different definitions conflict where they
should merge cleanly, and the conflict marker can land mid-definition. Those
files get Git's line-level merge instead, which is the better answer until the
parser gains a real per-definition model for them.

That is what the merge engine can parse. `weave setup` writes `.gitattributes`
rules for a narrower set, so Kotlin, HCL/Terraform, Vue, Svelte, ERB, CSV, Perl,
OCaml and Zig files still take git's line merge until you add the rule yourself:

```bash
echo '*.kt merge=weave' >> .gitattributes   # same shape for any extension above
```

## Install

```bash
brew install weave
```

Or build from source (requires Rust). Two binaries, both required: `weave`
(the CLI you run — `setup`/`explain`/`check`/...) and `weave-driver` (the one
git itself invokes on every merge; `weave setup` fails without it on `PATH`):

```bash
git clone https://github.com/Ataraxy-Labs/weave
cd weave
cargo install --path crates/weave-cli      # the `weave` binary
cargo install --path crates/weave-driver   # the `weave-driver` binary git calls
```

Upgrading an existing source install? `cargo install` refuses to overwrite a
binary it didn't put there itself — add `--force` to either command above.

## Setup

In any Git repo:

```bash
weave setup
```

This configures Git to use weave for all supported file types. Then use `git merge` as normal.

To revert back to normal git merging:

```bash
weave unsetup
```

To set up for just yourself (without modifying `.gitattributes`), write the same supported file type rules to `.git/info/attributes` instead:

```bash
weave setup --local
```

### Global (every repo)

To make weave the default merge driver for **all** your repos at once (no per-repo setup, like [mergiraf](https://mergiraf.org/usage.html#registration-as-a-git-merge-driver)):

```bash
weave setup --global
```

This writes the driver to your `~/.gitconfig` and the supported file-type rules to git's global attributes file (`~/.config/git/attributes`, or your `core.attributesfile` if set). No git repo required. Make sure `weave-driver` is on your `PATH` (it ships next to the `weave` binary).

The equivalent manual config, if you prefer:

```bash
git config --global merge.weave.name "Entity-level semantic merge"
git config --global merge.weave.driver "weave-driver %O %A %B %L %P"
# then add `*.ts merge=weave` (etc.) to ~/.config/git/attributes
```

## Jujutsu (jj)

Add to your jj config (`jj config edit --user`):

```toml
[merge-tools.weave]
program = "weave-driver"
merge-args = ["$base", "$left", "$right", "-o", "$output", "-l", "$marker_length", "-p", "$path"]
merge-conflict-exit-codes = [1]
merge-tool-edits-conflict-markers = true
conflict-marker-style = "git"
```

Resolve conflicts with `jj resolve --tool weave`, or set as default:

```bash
jj config set --user ui.merge-editor "weave"
```

## Preview

Dry-run a merge to see what weave would do:

```bash
weave preview feature-branch
```

```
  src/utils.ts — auto-resolved
    unchanged: 2, added-ours: 1, added-theirs: 1
  src/api.ts — 1 conflict(s)
    ✗ function `process`: both modified

✓ Merge would be clean (1 file(s) auto-resolved by weave)
```

After a real conflict, `weave explain <file>` and `weave check` are the
next two commands — see [Quickstart](#quickstart).

## MCP Server

For agent frameworks that speak [MCP](https://modelcontextprotocol.io):

```bash
# Claude Code
claude mcp add --scope user weave -- weave-mcp

# Any MCP client, via stdio (~/.config/claude/claude_desktop_config.json etc.)
{ "mcpServers": { "weave": { "command": "weave-mcp" } } }
```

The server discovers the repo from the first tool call's file path, the
`WEAVE_REPO` env var, or its working directory. It exposes `weave_check` and
`weave_findings` as the read contract for acting on a merge, entity
inspection tools (`weave_extract_entities`, `weave_diff`,
`weave_get_dependencies`/`_dependents`), and a claim/release layer for
coordinating multiple agents in one repo. Each tool's own description states
when to call it and what an empty result means.

## Architecture

```
weave-core       # Library: entity extraction, 3-way merge algorithm, reconstruction
weave-driver     # Git merge driver binary (called by git via %O %A %B %L %P)
weave-cli        # CLI: `weave setup`, `weave explain`, `weave check`, `weave preview`, ...
weave-crdt       # Automerge-backed multi-agent coordination state
weave-mcp        # MCP server exposing weave to agent frameworks
```

Uses [sem-core](https://github.com/Ataraxy-Labs/sem) for entity extraction via tree-sitter grammars.

## How It Works

```
         base
        /    \
     ours    theirs
        \    /
       weave merge
```

1. **Parse** all three versions into semantic entities via tree-sitter
2. **Extract regions** — alternating entity and interstitial (imports, whitespace) segments
3. **Match entities** across versions by ID (file:type:name:parent)
4. **Resolve** each entity: one-side-only changes win, both-changed attempts intra-entity 3-way merge
5. **Reconstruct** file from merged regions, preserving ours-side ordering
6. **Fallback** to line-level merge for files >1MB, binary files, or unsupported types

## Star History

[![Star History Chart](https://star-history.dera.page/svg?repos=Ataraxy-Labs/weave&type=Date)](https://star-history.dera.page/#Ataraxy-Labs/weave&Date)
