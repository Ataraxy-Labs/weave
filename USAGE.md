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
  <a href="#how-weave-land-works">How weave land works</a> ·
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

## How weave land works

`weave land` takes a merge from "git stopped" (or "an agent wants this on main") to published, in four steps:

1. **Merge.** weave merges by function and by entry, not by line. Two agents who add different entries to the same list, table, map or `switch` both keep their entry.
2. **Gate.** Every file of the merge must pass fixed, built-in rules: nothing either side wrote is dropped, nothing either side changed is undone, no key or `case` label is stated twice, and the file still parses. The gate is deterministic code, the same on every run, with no AI in it.
3. **Verify.** Your command runs on the merged tree (`--verify-cmd 'cargo test'`), and/or `--check sem` runs `sem check`: the project's own compiler, type checker, linter and tests, rechecking only what the merge can affect when that gives the same answer. Any verdict but pass refuses.
4. **Publish.** Only if the gate and the verify step both pass, fast-forward only. If main moved meanwhile, the new main is merged in and all four steps run again.

What is fixed and what is yours: the gate's rules are fixed and cannot be turned off. What runs in the verify step is configurable.

**Example: two agents add a case to the same `switch`.** Agent A adds `case 500`, agent B adds `case 409`, both right after `case 404`. git conflicts on those lines; weave keeps both:

```bash
git merge agent-b      # CONFLICT (content): Merge conflict in status.go
weave land
```
```
weave land: 1 file(s) git could not merge — 0 PROVEN, 1 VERIFIED, 0 REFUSED
  VERIFIED  status.go  elem_union: weave's element union passed the independent element check (both-changed regions admitted by: elem_union) and the gate; no resolver was asked
```
```go
	case 404:
		return "not found"
	case 409:
		return "from agent b"
	case 500:
		return "server error"
```

**The same, but both agents add `case 500`** with different bodies. That is a real conflict: only a person or a resolver can say which body is right. If a resolver answers by keeping both cases, the gate refuses the answer, and the agent gets:

```
weave land: 1 file(s) git could not merge — 0 PROVEN, 0 VERIFIED, 1 REFUSED
  REFUSED   status.go  gate: the resolver's answer failed the gate twice: WEAVE
              - DUP: the case `500` is stated 2x in one `switch code {`; ours states it 1x there, theirs 1x
```

A refused file keeps its conflict markers and nothing is published (exit 1).

To land onto main:

```bash
weave land --onto origin/main --verify-cmd 'go build ./... && go test ./...'
weave land --onto origin/main --check sem
```

## Quickstart

```bash
weave setup                 # this repo now merges through weave; git merge/rebase/cherry-pick unchanged
git merge <branch>          # real conflicts land as markers with a `refused_by:` line stating why
weave explain <file>        # per-hunk detail for one conflicted file, read off the actual git stages
#  ...edit to resolve...
weave check                 # verify the working tree against the three merge stages; exits 1 on findings, 2 if it could not verify
weave land --resolver <cmd> # or: land it, each conflicted file labelled PROVEN / VERIFIED / REFUSED
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

31 hand-crafted merge scenarios across 7 languages, comparable to [mergiraf](https://mergiraf.org/)'s own test corpus. Run `weave stats --bench` to reproduce:

Two of the 31 **must not** merge. When both sides add different decorators to the same Python or TypeScript function, decorator application is function composition, so the stack order is a semantic decision neither side made — `@cache` outside `@auth` serves cached responses without ever running the auth check. Weave refuses rather than fabricate an order, and a tool that merges those cleanly is wrong, not better. Annotations in Java, C# and Kotlin are unordered metadata, so weave still set-unions those.

| Tool | Clean merges (of 29 mergeable) | Correct outcomes (of 31) |
|------|-------------------------------|--------------------------|
| **Weave** | **29/29** (100%) | **31/31** (100%) |
| Mergiraf (v0.16.3) | 26/29 (90%) | 28/31 (90%) |
| Git | 15/29 (52%) | 17/31 (55%) |

All three tools correctly refuse the two decorator scenarios. Mergiraf fails on both-add-at-end-of-file and insert-between-existing; weave resolves those because it operates at entity granularity (functions, classes, methods) rather than AST node level. Full breakdown at [ataraxy-labs.github.io/weave](https://ataraxy-labs.github.io/weave/benchmarks.html).

## Real-World Benchmarks

Replayed against real merge commits from five long-lived open-source repositories. For each of the first 500 merge commits per repo, weave re-runs the merge (base/ours/theirs from the actual git history) and compares its output to both Git's line merge and the human-authored merge commit. Reproduce with `weave stats --repo <path-to-clone> --limit 500`; full per-repo breakdown, including which files disagree and why, is at [ataraxy-labs.github.io/weave/benchmarks.html](https://ataraxy-labs.github.io/weave/benchmarks.html).

- **Win**: the line-based 3-way merge conflicted, weave resolved cleanly
- **Regression**: the line-based 3-way merge resolved cleanly, weave conflicted
- **Human match**: of weave's wins, how many are byte-identical (whitespace-normalized) to what the developer actually wrote

> **Note (0.5.3):** regenerated on the 0.5.3 engine on 2026-09-01 (`weave stats --repo <clone>
> --limit 500`, fresh full clones). Read against the previous table with three caveats, stated
> rather than smoothed over. First, 0.5.3 conflicts on purpose where 0.5.2 sometimes resolved
> silently (divergent same-name additions, tightened same-entity and gap verdicts) — most of the
> regression increase is that tightening doing its job; on CPython, whose tested window is
> identical between runs, all of it is. Second, the earlier run's exact commit window was not
> recorded, and `stats --repo` walks the most recent N merges — for git, Go, and TypeScript the two
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

INI files (`*.ini`, `*.cfg`) have no grammar but are claimed too
(`weave_core::LINE_RULE_EXTENSIONS`): they take Git's line merge, plus the
rule that unites keys two sides added to one section.

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
weave setup --off
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

### Declaring a container a set (`weave-set`)

When two branches each add an entry at the same point of one container,
weave unites them only if the container's order carries no meaning: a
Python dict or JS/TS object literal with constant keys, `__all__`, an
`import`/`export` name list, the keys of one INI section, TOML/JSON/YAML
keys. The union never depends on which side is "ours", and in a Python,
JS/TS or INI container it is merged by key, so a fleet of branches that
each add one entry lands on one tree whatever order they merge in (an entry
under its own comment keeps its run together instead).

A list, a statement sequence, match/switch arms, an INI multi-line value or
a YAML sequence is ordered in general (the first matching arm wins, a
parametrize list names test ids by position), so two insertions at one point
stay a conflict. So do the entries two branches add to one section of a
changelog: two differently worded bullets can describe one change, and
telling them apart is not a set's job. If you know one of yours is really a
set, say so with the `weave-set` gitattribute:

```gitattributes
setup.cfg              weave-set=console_scripts
tests/test_*.py        weave-set=parametrize
src/cli.py             weave-set=build_parser
.pre-commit-hooks.yaml weave-set
CHANGELOG.md           weave-set=Unreleased
```

A bare `weave-set` makes every container in the file a set; a value is a
comma-separated list of container names: the variable a literal is assigned
to, the key it is the value of, the function it is an argument of
(`parametrize`), the function whose body or match arms it is, the INI key of
a multi-line value, the YAML/JSON/TOML key of an array, the title of a
changelog section or of a heading above it (brackets off, or its first word:
`## [Unreleased]` is `Unreleased`, `## 1.2.0 - 2026-01-01` is `1.2.0`). The driver reads the
attribute with `git check-attr` only when such a container is what stands
between a conflict and a union. Every union is still checked: an entry
edited, deleted or moved beside the insertion, one key with two values, or
an answer that does not parse stays a conflict.

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

## Landing an agent's merge: `weave land`

`weave land` runs where `git merge` stopped and gives every file git could not merge
(content, add/add and modify/delete conflicts) one of three labels:

| Status | Meaning |
|---|---|
| **PROVEN** | weave merged the file cleanly **and** an independent merge certificate (`crates/weave-certify`, which shares no merge code with weave) shows the result is the three-way selection of base, ours and theirs. Where both sides changed one region, only the rules `imp_used` (import lines, each one a side added still used by the merged file), `subsume_ins` and `nest` can admit it. |
| **VERIFIED** | The answer passed the exact gate below, and either a resolver of your choice wrote it, or weave merged the file cleanly as an element union (two sides adding entries to one map, switch, const block or test table) that the certificate's own `elem_union` check admits — every inserted element verbatim, base order kept, no key twice, a construct the policy treats as a set, a result that parses. The check also reads the whole file as one statement list, so both sides appending distinctly named top-level functions (Go `func`/types/methods, JS `function`, Python `def`) lands the same way. The second needs no resolver call; it is VERIFIED, not PROVEN, because `elem_union` is not part of the proof rule set. |
| **REFUSED** | Neither. The file keeps its conflict markers and stays unmerged, so `git commit` refuses. A rejected answer is never written. |

```bash
git merge agent/feature
weave land --resolver 'WEAVE_LAND_MODEL="<any model CLI>" python3 scripts/weave-land-resolver.py' \
           --certificate land.json
git commit                   # only once nothing is REFUSED
```

**The resolver** is any command, run with `sh -c`. It gets one JSON object on stdin: `path`,
`kind`, `base`, `ours`, `theirs`, `conflicted` (git's merge with markers), `attempt`, `previous`
and `findings`, with `null` for an absent side. It prints the complete resolved file, or one line:
`DELETE` or `KEEP` (modify/delete only), or `CANNOT[: reason]`. Nothing is tied to a model vendor.
`scripts/weave-land-resolver.py` is an example that prompts whatever CLI `WEAVE_LAND_MODEL` names.

**The gate** checks each answer against the file's three stages:

- no conflict-marker line that neither side has;
- it parses when both sides parse;
- `weave check` finds nothing: lost or duplicated lines, dangling names, duplicate data keys, a
  data file that no longer loads, modify/delete. A duplicate or dangling name that ours or theirs
  already has doesn't count against the answer. A file weave has no grammar for gets the line
  rules alone.
- every line git merged automatically outside the conflict blocks is still there;
- inside each conflict block, both sides' changes survive: every token (identifier, number,
  operator) one side added there is still there, and nothing one side deleted there is back. An
  answer that keeps one side of a block verbatim is refused unless that side already holds the
  other's change; identical changes and one side subsuming the other pass, and a block where one
  side deletes what the other edits is refused whichever side is kept. Tokens, not lines, so
  re-wrapping or folding two edits of one line into one line passes.

A failed answer gets one retry, with the findings fed back. It passes (VERIFIED) or it is
refused, with the findings.

In a blind audit (n = 200 accepted files per arm), a cheap model resolving agent-PR
conflicts on its own had 22.0% of its accepted merges judged wrong. Behind this pipeline the
figure was 9.5%. That gate did not have the both-sides rule (the last bullet), which was added
after a one-sided answer was seen passing it.

**An answer already in the tree** is judged before any resolver is asked: a file the merge
driver resolved by itself (stage 0 in the index), or, with `--result <rev>`, that revision's
file. It is PROVEN when it is weave's certified merge and VERIFIED when it passes the gate. So
a file weave's driver merged cleanly is not handed back to the resolver as if it were
unresolved.

The labels cover files git's line merge conflicts on, and every other file both sides changed
whose merge does not pass weave's own merge check. A file git merges line-cleanly is examined
too: git's answer must lose no line both sides kept, state nothing twice, and state no `case`
label, map/object/dict key or struct-literal field twice in one container where neither side
does (two features each adding `case 3:` to one switch is line-clean to git and does not
compile). One that fails is a unit like a conflicted file. Each file is checked on its own
three stages, so an effect across files (a caller in a file neither side touched) is outside
the gate, and so is a semantic conflict that leaves every file well-formed — that is what
`--verify-cmd` is for. Renames are not followed.

| Flag | |
|---|---|
| `--resolver <cmd>` | The resolver. Without one, every unproven file is REFUSED. |
| `--json` | Print the report as JSON (`schema: "weave-land/1"`). |
| `--certificate <file>` | Write the same JSON as the review certificate for the merge commit. It records the three commits and each file's status, rule, reason, findings and landed sha256. |
| `--dry-run` | Decide and report, but write nothing. |
| `--base/--ours/--theirs <rev>` | Read a merge between revisions instead of the one in progress. Implies `--dry-run`; useful in CI. |
| `--result <rev>` | With the above: judge `<rev>`'s files as the merge's answer (re-check a merge commit as committed). |

Exit codes: `0` every file PROVEN or VERIFIED, `1` some file REFUSED, `2` the run failed.

### The whole landing: `weave land --onto <remote>/<branch>`

`--onto` owns the loop an agent's `land` script used to: it fetches the branch, merges its tip
in (`git merge --no-commit`, through whatever merge driver is configured), runs the gate over
**every** file of that merge (conflicted, driver-merged and line-clean alike), commits it, gates
any other merge commit on the branch not yet checked (as committed, or as the branch now holds
the file, so a later fix counts), runs `--verify-cmd` on the final tree, and
then updates the branch on the remote fast-forward only. If the branch moved meanwhile, it merges
the new tip and checks everything again; nothing is ever published that has not passed the gate
and the verify command against the exact tip it lands on. Any refusal publishes nothing and
exits 1; a refused merge is left in progress with the refused files conflicted, and running
`--onto` again checks the in-place resolution and commits it.

```bash
weave land --onto origin/main --verify-cmd 'go vet ./... && go test ./...'
```

**Set a verify command.** The gate reads each file on its own; it cannot see that two features
which each merge cleanly no longer work together (in one benchmark run, two lexer changes that
both parsed and compiled made a new operator unreachable). At the least run a compile or build
(`go vet ./...` — it type-checks test files too —, `cargo check`, `tsc --noEmit`,
`npm run build`); better, the tests the merge touched. It runs with `sh -c` in the repository
root, on a clean checkout of the commit to be published, with `WEAVE_LAND_ONTO` (the tip landed
onto) and `WEAVE_LAND_HEAD` in the environment; a non-zero exit, a timeout, or a change to
tracked files refuses.

| Flag | |
|---|---|
| `--onto <remote>/<branch>` | Land the current branch there, as above. |
| `--verify-cmd <cmd>` | Run on the final tree before publishing; failure refuses. Recommended: at least a build. |
| `--verify-timeout <secs>` | Default 1800. |
| `--attempts <n>` | How many times to merge a moved tip and retry (default 5). |
| `--certificate-dir <dir>` | Write every gate certificate there. |
| `--resolver <cmd>` | As above, for files neither weave nor an in-place resolution answers. |

Checked merges are remembered by commit id (`.git/weave-land-verified`) and verify passes by
tree and command (`.git/weave-land-verify-ok`), so a retry repeats no work.

## CLI Commands

Seven commands. Each `--help` line says which question the command answers, and every flag has an example (`weave <command> --help`).

| Command | The question it answers |
|---|---|
| `weave setup [--global] [--local] [--off]` | Make `git merge` use weave here (`--global`: every repo; `--off`: stop) |
| `weave land [--resolver <cmd>] [--onto <remote>/<branch>] [--verify-cmd <cmd>] [--check sem] [--queue]` | Can this merge land? Merge, gate every file (PROVEN / VERIFIED / REFUSED), verify, publish; see [above](#landing-an-agents-merge-weave-land). `--queue` is experimental |
| `weave explain <file> [--summary] [--json]` | Why did this file conflict? `--summary`: a structured summary of the weave markers in any file |
| `weave check [--json]` | Is my conflict resolution right? Lost lines, duplicates, leftover markers, dangling names |
| `weave preview <branch> [--file <path>]` | What would merging this branch look like? Nothing is written |
| `weave patch extract <base-file> <changed-file>` / `weave patch apply <ops-file> <target-file>` | Apply an edit to a file that drifted: typed entity ops, merged three-way against the ops' base |
| `weave stats [--bench] [--repo <path> --limit N]` | How has weave done? Lifetime counters if you opted in with `WEAVE_STATS=1`; `--bench` runs the 31-scenario synthetic benchmark; `--repo` replays real merge commits from a clone (see [Real-World Benchmarks](#real-world-benchmarks)) |

The `weave experimental` group (hidden from `--help`) is the live-editing prototype: it claims and releases entities and applies edits held in the `.weave/state.automerge` CRDT document, the same document the MCP coordination tools below use. Claims are advisory. That document lives in the repo's working tree but is never repo content: the first time weave writes it, it adds `.weave/` to the repo's local `.git/info/exclude` (never your own `.gitignore`), so it never shows up in `git status` or gets swept into `git add -A`.

## MCP Server

For agent frameworks that speak [MCP](https://modelcontextprotocol.io):

```bash
# Claude Code
claude mcp add --scope user weave -- weave-mcp

# Any MCP client, via stdio (~/.config/claude/claude_desktop_config.json etc.)
{ "mcpServers": { "weave": { "command": "weave-mcp" } } }
```

The server discovers the repo from the first tool call's file path, the
`WEAVE_REPO` env var, or its working directory. It exposes 22 tools in three
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
via `weave_claim_entity`/`weave_update_entity_content` or `weave experimental claim`/`weave experimental apply`.
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
- **Entity claims are advisory, not enforced.** `weave_claim_entity` and `weave experimental claim` are
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
