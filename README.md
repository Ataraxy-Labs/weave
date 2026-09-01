> **Part of the [Ataraxy Labs](https://ataraxy-labs.com) stack**, agent-native infrastructure for software development. See also: [sem](https://ataraxy-labs.com/sem) (semantic version control) · [inspect](https://github.com/Ataraxy-Labs/inspect) (semantic code review) · [opensessions](https://github.com/Ataraxy-Labs/opensessions) (tmux sidebar for coding agents).
>
> Read the manifesto: https://ataraxy-labs.com/#thesis · Essays: https://ataraxy-labs.com/blogs · LLMs: https://ataraxy-labs.com/llms.txt

<p align="center">
  <img src="assets/banner.svg" alt="weave" width="600" />
</p>

<p align="center">
  <strong>Entity-level semantic merge for Git.</strong><br>
  Resolves merge conflicts that Git can't by parsing code into functions, classes, and keys with tree-sitter, then merging those entities instead of lines.
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#quickstart">Quickstart</a> ·
  <a href="#how-weave-fixes-this">How It Works</a> ·
  <a href="#mcp-server">MCP Server</a> ·
  <a href="#cli-commands">CLI</a> ·
  <a href="https://github.com/Ataraxy-Labs/weave/releases/latest">Releases</a>
</p>

<p align="center">
  <a href="https://github.com/Ataraxy-Labs/weave/releases/latest"><img src="https://img.shields.io/github/v/release/Ataraxy-Labs/weave?color=blue&label=release" alt="Release"></a>
  <a href="https://formulae.brew.sh/formula/weave"><img src="https://img.shields.io/badge/homebrew-weave-orange" alt="Homebrew"></a>
  <img src="https://img.shields.io/badge/rust-stable-orange" alt="Rust">
  <img src="https://img.shields.io/badge/tests-441_passing-brightgreen" alt="Tests">
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

See [Setup](#setup) for `--global`/`--local` variants, [CLI Commands](#cli-commands) for the rest of the
`weave` binary, and [MCP Server](#mcp-server) for agent-framework integration.

## The Problem

Git merges by comparing **lines**. When two branches both add code to the same file, even to completely different functions, Git sees overlapping line ranges and declares a conflict:

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

Weave replaces Git's line-based merge with **entity-level merge**, a 3-way merge that compares base, ours, and theirs at the level of individual functions, classes, and keys instead of individual lines. That lets it tell where the two branches actually drifted apart, rather than just where their edits happen to land on the same line numbers. It works like this:

1. Parses all three versions (base, ours, theirs) into semantic entities: functions, classes, JSON keys, etc., using [tree-sitter](https://tree-sitter.github.io/)
2. Matches entities across versions by identity (name + type + scope), including renames
3. Merges at the entity level:
   - **Different entities changed** → auto-resolved, no conflict
   - **Same entity changed by both** → attempts intra-entity merge, conflicts only if truly incompatible
   - **One side modifies, other deletes** → flags a meaningful conflict

Run the same scenario above through weave, and it merges cleanly with zero conflicts: both functions end up in the output.

This merge algorithm is deterministic and stateless: it reads three file revisions and writes one result, the same way `git merge-file` does. It is not a CRDT. (Weave separately ships a CRDT-backed coordination layer, `weave-crdt`, for tracking *live* multi-agent edits before they hit Git; see [Architecture](#architecture).)

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

31 hand-crafted merge scenarios across 7 languages, comparable to [mergiraf](https://mergiraf.org/)'s own test corpus. Run `weave bench` to reproduce:

| Tool | Clean Merges | Score |
|------|-------------|-------|
| **Weave** | **31/31** | 100% |
| Mergiraf (v0.16.3) | 26/31 | 83% |
| Git | 15/31 | 48% |

Mergiraf fails on both-add-at-end-of-file, insert-between-existing, and decorator conflict scenarios. Weave resolves all of these because it operates at entity granularity (functions, classes, methods) rather than AST node level. Full breakdown at [ataraxy-labs.github.io/weave](https://ataraxy-labs.github.io/weave/benchmarks.html).

## Real-World Benchmarks

Replayed against real merge commits from five long-lived open-source repositories. For each of the first 500 merge commits per repo, weave re-runs the merge (base/ours/theirs from the actual git history) and compares its output to both Git's line merge and the human-authored merge commit. Reproduce with `weave bench-repo <path-to-clone> --limit 500`; full per-repo breakdown, including which files disagree and why, is at [ataraxy-labs.github.io/weave/benchmarks.html](https://ataraxy-labs.github.io/weave/benchmarks.html).

- **Win**: the line-based 3-way merge conflicted, weave resolved cleanly
- **Regression**: the line-based 3-way merge resolved cleanly, weave conflicted
- **Human match**: of weave's wins, how many are byte-identical (whitespace-normalized) to what the developer actually wrote

> **Note (0.5.3):** regenerated on the 0.5.3 engine on 2026-09-01 (`weave bench-repo <clone>
> --limit 500`, fresh full clones). Read against the previous table with three caveats, stated
> rather than smoothed over. First, 0.5.3 conflicts on purpose where 0.5.2 sometimes resolved
> silently (divergent same-name additions, tightened same-entity and gap verdicts) — most of the
> regression increase is that tightening doing its job; on CPython, whose tested window is
> identical between runs, all of it is. Second, the earlier run's exact commit window was not
> recorded, and `bench-repo` walks the most recent N merges — for git, Go, and TypeScript the two
> runs replay substantially different commit sets, so cross-run rate comparisons there are
> indicative, not exact; this run's windows are current as of the date above. Third, audited
> details: the "clean line merge" baseline is a diff3 implementation (`diffy`), which disagrees
> with `git merge-file` on a small number of cases; 3 of the 86 regressions are a known guard
> false-positive (files whose *source code contains conflict-marker string literals* — e.g.
> TypeScript's own scanner); and in 2 regressions the line merge's "clean" result differs from
> what the human actually committed, i.e. weave's refusal was arguably the safer verdict.

| Repository | Language | File merges tested | Wins | Regressions | Human match |
|------------|----------|--------------------:|-----:|-------------:|-------------:|
| [git/git](https://github.com/git/git) | C | 1,701 | 183 | 23 | 72% |
| [Flask](https://github.com/pallets/flask) | Python | 67 | 15 | 1 | 33% |
| [CPython](https://github.com/python/cpython) | C / Python | 256 | 11 | 10 | 45% |
| [Go](https://github.com/golang/go) | Go | 1,667 | 120 | 37 | 33% |
| [TypeScript](https://github.com/microsoft/TypeScript) | TypeScript | 1,280 | 15 | 15 | 53% |

Across all five repos: 344 wins on 4,971 file merges (0.5.2 measured 83 wins on 4,517), with 86 total
regressions spread across every repo (0.5.2 measured 3, all on TypeScript). The jump in regressions is
expected and by design, not a quality drop we're hiding: 0.5.3 now conflicts on genuinely divergent
concurrent additions that 0.5.2 silently merged, and no longer resolves a case that could resurrect a
deleted JSON key — both changes move cases from "weave resolves" into "weave conflicts" under this
benchmark's own regression definition (git resolves cleanly, weave doesn't). Wins also rose substantially
on every repo. File-merge counts and human-match rates shifted too, partly because "first 500 merge
commits" is a moving window and all five repos have advanced since the 0.5.2 run. See the per-repo
breakdown on the benchmarks page before relying on weave for large merges in any of these languages.

## Testing

The open test suite in this repository, 441 unit and integration tests plus a five-scenario
sweep per supported language in `crates/weave-core/tests/language_coverage.rs`, covers the
documented merge properties and runs in CI (`cargo fmt --check`, `cargo clippy -D warnings`,
`cargo test --workspace`) on Linux and Windows on every push and PR.

## Conflict Markers

When a real conflict occurs, weave gives you context that Git doesn't: which
entity, what type, and, on the line inside the box, which internal guard
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
three merge stages; see [Quickstart](#quickstart).

## Supported Languages

TypeScript, TSX, JavaScript, Python, Go, Rust, Java, C, C++, Ruby, C#, PHP, Swift, Kotlin, Scala, Dart, Elixir, Bash, Fish, Fortran, Perl, OCaml, Zig, Elm, Clojure, EDN, D, Lua, Nix, SQL, HCL/Terraform, LaTeX, XML, JSON, YAML, TOML, CSV, Markdown. Falls back to standard line-level merge for everything else.

`weave setup` derives its `.gitattributes` rules directly from the parser
registry: every extension the tree-sitter grammars recognize gets a
`merge=weave` line automatically, with no hand-maintained list to fall
behind. Each language on it passes a five-scenario merge sweep (two sides
adding different definitions merges clean, two sides rewriting the same
definition conflicts, and nothing is dropped) in
`crates/weave-core/tests/language_coverage.rs`, and a separate parity test
(`crates/weave-core/tests/setup_extension_coverage.rs`) fails the build if a
newly added grammar is ever left unclaimed and undeclined.

Vue, Svelte, ERB and Haskell are parsed but deliberately **not** claimed:
weave declines exactly these four (`weave_core::DECLINED_EXTENSIONS`), and
nothing else. Their entity model treats a whole `<script>` block, template,
or type signature as a single unit, so two people adding two different
definitions conflict where they should merge cleanly, and the conflict
marker can land mid-definition. Those files get Git's line-level merge
instead, which is the better answer until the parser gains a real
per-definition model for them. They're excluded from `weave setup`
automatically; nothing to opt out of by hand.

## Install

```bash
brew install weave
```

Or build from source (requires Rust). Two binaries, both required: `weave`
(the CLI you run: `setup`/`explain`/`check`/...) and `weave-driver` (the one
git itself invokes on every merge; `weave setup` fails without it on `PATH`):

```bash
git clone https://github.com/Ataraxy-Labs/weave
cd weave
cargo install --path crates/weave-cli      # the `weave` binary
cargo install --path crates/weave-driver   # the `weave-driver` binary git calls
```

Upgrading an existing source install? `cargo install` refuses to overwrite a
binary it didn't put there itself, so add `--force` to either command above.

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
next two commands; see [Quickstart](#quickstart).

## CLI Commands

Beyond `setup`/`explain`/`check`/`preview` above, the `weave` binary has commands for the
CRDT coordination layer and for typed entity patches. Run `weave --help` or `weave <command> --help`
for the full flag list; the table below is what each one is for.

| Command | What it does |
|---|---|
| `weave status [--file] [--agent]` | Entity and agent state from the CRDT: claims, last editor, merge state |
| `weave claim <agent-id> <file> <entity>` | Claim an entity before editing it (advisory: weave does not enforce it) |
| `weave release <agent-id> <file> <entity>` | Release a previously claimed entity |
| `weave apply <file>...` | Materialize entity edits held in the CRDT back onto the working files |
| `weave patch extract <base-file> <changed-file>` | Emit the typed ops that turn `base-file` into `changed-file` |
| `weave patch apply <ops-file> <target-file>` | Apply those ops to a target file, three-way against the ops' base, in case the target has drifted since the ops were extracted |
| `weave summary <file>` | Parse a file's weave conflict markers into a structured (optionally JSON) summary |
| `weave stats` | Lifetime merge counters, if you've opted in with `WEAVE_STATS=1` (off by default) |
| `weave bench` | Run the 31-scenario synthetic benchmark against weave, Mergiraf, and git |
| `weave bench-repo <path> [--limit N]` | Replay real merge commits from a cloned repo; see [Real-World Benchmarks](#real-world-benchmarks) |

`claim`/`release`/`status`/`apply` all operate on the same `.weave/state.automerge` CRDT
document as the MCP tools below: the CLI and MCP server are two front ends onto one
coordination state. That document lives in the repo's working tree but is never repo
content: the first time weave writes it, it adds `.weave/` to the repo's local
`.git/info/exclude` (never your own `.gitignore`), so it never shows up in `git status`
or gets swept into `git add -A`.

## MCP Server

For agent frameworks that speak [MCP](https://modelcontextprotocol.io):

```bash
# Claude Code
claude mcp add --scope user weave -- weave-mcp

# Any MCP client, via stdio (~/.config/claude/claude_desktop_config.json etc.)
{ "mcpServers": { "weave": { "command": "weave-mcp" } } }
```

The server discovers the repo from the first tool call's file path, the
`WEAVE_REPO` env var, or its working directory. It exposes 22 tools in two
independent groups (each tool's own description states when to call it and
what an empty result means):

- **Merge analysis** reads git refs or the working tree directly, no setup needed:
  `weave_findings`, `weave_check`, `weave_preview_merge`, `weave_diff`,
  `weave_merge_audit`, `weave_validate_merge`, `weave_merge_summary`.
- **Entity and dependency inspection** reads a file's or the repo's structure:
  `weave_extract_entities`, `weave_get_dependencies`, `weave_get_dependents`,
  `weave_impact_analysis`.
- **Live coordination** tracks edits in the shared CRDT (`.weave/state.automerge`) for
  agents editing the same repo at the same time, starting with `weave_agent_register`:
  `weave_agent_register`, `weave_agent_heartbeat`, `weave_claim_entity`,
  `weave_release_entity`, `weave_status`, `weave_who_is_editing`,
  `weave_potential_conflicts`, `weave_update_entity_content`,
  `weave_get_entity_content`, `weave_merge_file`, `weave_resolve_conflict`.

Start with `weave_findings` after (or before) a merge between two branches, or
`weave_check` for the cross-file binding risk a per-file git merge driver can't see:
a rename in `a.py` whose surviving caller lives in `b.py` merges both files cleanly on
its own, and the break is only visible repo-wide.

## Architecture

```
weave-core       # Library: entity extraction, entity-level 3-way merge, reconstruction
weave-driver     # Git merge driver binary (called by git via %O %A %B %L %P)
weave-cli        # CLI: `weave setup`, `weave explain`, `weave check`, `weave patch`, ...
weave-crdt       # Automerge-backed CRDT: live multi-agent coordination state only
weave-mcp        # MCP server exposing weave to agent frameworks (22 tools)
weave-github     # GitHub webhook service behind the hosted PR-comment integration
                 #   (publish = false, not a binary you install; runs weave's merge
                 #   analysis on pull_request events and posts the result as a comment)
```

Uses [sem-core](https://github.com/Ataraxy-Labs/sem) for entity extraction via tree-sitter grammars.

The merge algorithm (`weave-core`) and the coordination state (`weave-crdt`) are
deliberately separate concerns with different data models: the merge is a pure
function over three file revisions, run fresh on every `git merge`/`weave preview`/
`weave check` call. The CRDT is the thing that persists; it's what lets two live
agents see each other's claims and in-flight edits *before* either one commits,
via `weave_claim_entity`/`weave_update_entity_content` or `weave claim`/`weave apply`.
Nothing in the merge path depends on the CRDT ever having run.

## How It Works

```
         base
        /    \
     ours    theirs
        \    /
       weave merge
```

1. **Parse** all three versions into semantic entities via tree-sitter
2. **Extract regions**, alternating entity and interstitial (imports, whitespace) segments
3. **Match entities** across versions by ID (file:type:name:parent), detecting renames
4. **Resolve** each entity: one-side-only changes win, both-changed attempts intra-entity 3-way merge
5. **Reconstruct** file from merged regions, preserving ours-side ordering
6. **Fallback** to line-level merge for files >1MB, binary files, or unsupported types

## Limitations

- **Vue, Svelte, ERB, Haskell** parse, but their per-file entity model is too coarse to merge
  well, so they take Git's line merge, always (see [Supported Languages](#supported-languages)).
- **Files over 1MB, binary files, and file types with no parser** fall back to Git's line-level
  merge automatically.
- **Entity claims are advisory, not enforced.** `weave_claim_entity` and `weave claim` are
  cooperative locks inside the CRDT coordination layer; weave does not stop a second agent
  (or you) from editing a claimed entity anyway.
- **Crashed agents aren't reaped.** `weave_agent_heartbeat`'s liveness timestamp is informational;
  weave does not currently expire or release a claim automatically when an agent stops
  heartbeating, so a crashed agent's claims stay visible until another call to
  `weave_release_entity`/`weave release`.
- **`weave stats` is empty until you opt in.** Lifetime merge counters are off by default; set
  `WEAVE_STATS=1` in the environment your git/jj merges run in to start accumulating them.

## Contributing

Bug reports and issues are welcome. This is a small team maintaining a merge engine that
runs inside other people's git workflows: incoming PRs get read, but are reviewed and
adapted before merge rather than merged as-is. Open an issue first for anything beyond a
small, obviously-correct fix, so the approach can be agreed on before you write the code.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option.

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Ataraxy-Labs/weave&type=Date)](https://star-history.com/#Ataraxy-Labs/weave&Date)
