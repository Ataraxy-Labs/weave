# weave

Entity-level semantic merge driver for Git. Resolves conflicts at the
function/class level instead of the line level, so two changes to different
functions in the same file never conflict.

## Setup

```bash
weave setup            # this repo: registers weave as the merge driver + .gitattributes
weave setup --global   # every repo on this machine, no per-repo step
weave setup --local    # this repo only, writes .git/info/attributes instead
```

`weave setup` auto-detects the `weave-driver` binary, writes
`merge.weave.driver` to git config, and adds `*.ts merge=weave` (etc.) for
every supported extension. The equivalent by hand, if you need it:

```bash
git config merge.weave.name "Entity-level semantic merge"
git config merge.weave.driver "weave-driver %O %A %B %L %P"
echo '*.py merge=weave' >> .gitattributes   # repeat per extension you want covered
```

After setup, `git merge` / `git rebase` / `git cherry-pick` all route through
weave automatically. No new commands to run to get a merge — only to resolve
one that still conflicts.

## The resolve loop

Most merges finish clean with no conflict markers. When one entity is
genuinely disputed by both sides, weave leaves a marker box like git's, plus
one line stating *why*:

```
<<<<<<< ours — function `process` (T, confidence: high)
// refused_by: statement_fold · collision: `    return data.upper()` +1 more
def process(data):
    return data.upper()
=======
def process(data):
    return json.dumps(data)
>>>>>>> theirs — function `process` (T, confidence: high)
```

1. **Read the `refused_by:` line first.** It names the guard that declined to
   auto-merge and quotes the exact line(s) both sides disagree about — that's
   usually enough to resolve on the spot.
2. **Still unclear? Run `weave explain <file>`.** It reads the three merge
   stages straight out of git's index and reports, per conflicted entity, the
   guard, the confidence, and the hunks *both* sides actually touched —
   narrower and more precise than diffing the whole file by eye.
3. **Edit.** Pick a side, merge by hand, or rewrite — same as any conflict.
   Remove the marker lines. The `weave explain` hint is written on the first
   box's closing marker, so it goes when the markers go. Older weave versions
   appended it as a separate trailing comment; `weave check` flags a leftover
   copy.
4. **Run `weave check`.** With no arguments it verifies the working tree —
   the file as you just edited it — against the three merge stages: markers
   left behind, lines both sides kept that went missing, anything stated more
   often than either side stated it, references that no longer resolve. One
   verdict line per file, always a sentence, never a bare empty result:

   ```
   OK: src/api.py — markers cleared, no unanimous-line loss, no duplicated
   definitions or lines, no dangling references
   ```

   A file with a real problem gets `FOUND: <file> — <what and where>` plus a
   suggested repair when one follows mechanically from the finding. Exit code
   is 0 clean / 1 findings, so it composes with a script or a pre-commit hook.

`weave check --base <rev> --ours <rev> --theirs <rev>` runs the same checks
between two revisions instead of the working tree (useful pre-merge, or with
no git repo at all via `--base-dir`/`--ours-dir`/`--theirs-dir`), and emits
JSON findings instead of the verdict sentence.

## MCP server (for agent frameworks)

```bash
claude mcp add --scope user weave -- weave-mcp
```

Or any MCP client, via stdio:

```json
{ "mcpServers": { "weave": { "command": "weave-mcp" } } }
```

`weave-mcp` discovers the repo from the first tool call's file path, the
`WEAVE_REPO` env var, or its working directory — set `WEAVE_REPO` if you
launch it from outside the repo. It lists one tool per command:
`weave_preview`, `weave_explain`, `weave_check`, `weave_land` and `weave_patch`.
For callers and blast radius use sem's MCP server. The live claim/release CRDT
layer (`weave_claim_entity`, `weave_status`, ...) is listed only with
`WEAVE_EXPERIMENTAL=1`. Each tool's description says when to call it and what an
empty result means — read that at call time, not here.

## Reference

| Command | Does |
|---|---|
| `weave setup` / `setup --off` | register / remove the git merge driver |
| `weave land` | land a merge: gate each conflicted file PROVEN / VERIFIED / REFUSED; `--onto origin/main --check sem` merges, gates, verifies and publishes |
| `weave explain <file>` | per-entity conflict detail for one conflicted file |
| `weave check` | verify the working tree (or two revisions) against the merge |
| `weave preview <branch>` | dry-run a merge before running it |
| `weave patch extract/apply` | typed entity ops — turn a diff into ops, apply them three-way to a drifted target |
| `weave stats` | lifetime counters; `--bench` / `--repo <path>` compare weave with git |
| `weave explain <file> --summary` | structured JSON summary of the markers in a file |
| `weave experimental status` / `claim` / `release` | CRDT-backed multi-agent coordination state (prototype) |

Run `weave --help` or `weave <command> --help` for full flags.

### Watching a lot of merges at once

`WEAVE_EVENT=1` makes the merge driver write one JSON line per merge to
stderr, behind a `weave-event: ` prefix:

```text
weave-event: {"schema":"weave-event","schema_version":"1.0.0","file":"src/app.py",
"outcome":"clean","exit_code":0,"confidence":"very_high","conflicts":0,"findings":0,
"entities":{...},"bytes_out":481,"ms_merge":4.43,"ms_total":5.72,...}
```

One line per merge, including the ones that fail — so "which files conflicted
in this rebase, on what, and how long did each take" is one pass over the
lines rather than four stderr channels joined by hand. Off by default; nothing
is computed for it that the driver did not already have. The fields are
described in `crates/weave-mcp/schema/weave-event.schema.json`.

## Supported languages

38 languages and data formats — TypeScript, JavaScript, Python, Go, Rust,
Java, C/C++, Ruby, C#, PHP, Swift, Kotlin, SQL, Lua, Nix, JSON, YAML and more
(see README for the full list). Anything else falls back to git's standard
line-level merge, including `.vue`, `.svelte`, `.erb` and `.hs`, which weave
can parse but does not yet merge well enough to claim.

## Build & test (contributors)

```bash
cargo build --release && cargo test --workspace   # weave, weave-driver, weave-mcp
```

License: MIT OR Apache-2.0.
