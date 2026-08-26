# Changelog

This file starts at 0.4.0. For earlier releases see the
[GitHub releases page](https://github.com/Ataraxy-Labs/weave/releases).

Versions are shared across every crate in the workspace and the npm package,
so `weave-core`, `weave-crdt`, `weave-driver`, `weave-cli`, `weave-mcp`,
`weave-github` and `@ataraxy-labs/weave` all move together.

## 0.5.3

### Fixed

- Renaming an entity between claiming it and updating or releasing that claim used to break coordination: `weave_update_entity_content` and `weave_release_entity` resolved the entity by name against the file's *current* content, so a rename in between made the old name unresolvable — the call failed with "entity not found" even though the claim was still held. `weave_claim_entity`'s response now includes an `entity_id`, the claim's own stable identity. Pass it to `weave_update_entity_content` or `weave_release_entity` and they address the claim directly, independent of whatever the entity is named now. Existing callers that only send `entity_name` are unaffected — resolution falls back to exactly today's behavior when `entity_id` is omitted.

## 0.5.2

sem-core bumped to 0.23.0. Entity identity and ids are unchanged for merges —
this is a dependency bump, not a behavior change in how entities are
recognized or matched.

### New — typed entity addressing

MCP tools and the CLI's `claim`/`release` commands used to resolve an entity
by name alone, so two entities sharing a name meant whichever one happened to
be found first — silently. `weave_crdt::resolve::EntityAddress` now carries an
optional `entity_type`, `parent_name`, and `ordinal` alongside the name.
MCP's six content-mutating tools (`claim_entity`, `release_entity`,
`who_is_editing`, `update_entity_content`, `get_entity_content`,
`resolve_conflict`) and its three read-only graph tools
(`get_dependencies`, `get_dependents`, `impact_analysis`) all take the new
optional fields; existing callers that only send a name are unaffected. An
ambiguous name is now refused with the list of candidates instead of being
resolved to the first match.

### Fixed

- A merge could splice an inserted statement above the binding it reads, when
  the other side had edited the same function around it. Gap insertions are
  now anchored after the matched statements they follow, so an inserted line
  lands where it was written relative to what's still there.

## 0.5.1

sem-core bumped to 0.21.1.

### New

- `weave check` with no arguments re-scanned the whole tree on every run —
  a `git show` per file plus a dangling-reference recompute across every
  subject, which went from 59 seconds to effectively hanging on a large
  monorepo. It's now scoped to the files a merge actually touched, with
  batched reads instead of one process per file: 59.3s to 0.17s on a
  4,000-file, 50-conflict tree, with an identical verdict.
- `weave setup` emitted a hardcoded list of extensions it would install merge
  drivers for, which had drifted from what the parser actually supports —
  missing `.mts`/`.cts` and about thirty others. The list is now derived from
  the parser registry itself, so the two can't drift apart again.
- When a container (class, impl, object, trait, enum) merges clean but each
  side changed or added a *different* set of sibling members, weave now
  emits a clean-merge advisory naming the co-changed siblings, instead of
  staying silent about a merge that was clean but still worth a second look.

### Fixed

- A stale peer clock could push the CRDT's staleness check backwards instead
  of holding its ground; it now saturates rather than trusting whatever a
  peer reports.
- A contested entity claim used to resolve silently; contested claims are now
  visible instead of picked for the caller.
- The sync door merged into the writes register in some paths and replaced it
  in others; it now always merges, never replaces.
- A failed git read inside the MCP server used to come back as fabricated
  empty content; it's now reported as a tool error.
- Git reads now run in the repository they were asked about, instead of
  whatever the process's ambient working directory happened to be.
- A git refusal (e.g. an unrelated-history diff) used to be answered with an
  empty change list, which read as "nothing changed" instead of "the request
  couldn't be answered."

## 0.5.0

### New — one line per merge

Set `WEAVE_EVENT=1` and the merge driver writes one JSON line per merge to
stderr, behind a `weave-event: ` prefix:

```text
weave-event: {"schema":"weave-event","schema_version":"1.0.0","file":"src/app.py",
"outcome":"clean","exit_code":0,"confidence":"very_high","conflicts":0,"findings":0,
"entities":{...},"bytes_out":481,"ms_merge":4.43,"ms_total":5.72,...}
```

It answers the question a rebase raises — which files conflicted, on what, and
how long each took — in one pass over the lines instead of four stderr channels
joined by hand. A line is written for every outcome, including the ones that
produce nothing, because a channel that only records successes cannot explain a
bad run. Off by default, and everything on the line was already computed:
turning it on costs one line and no extra work. Fields are documented in
`crates/weave-mcp/schema/weave-event.schema.json`.

### Breaking — library API

Nothing here affects the CLI, the merge driver or the MCP server. These change
Rust code that depends on `weave-core` or `weave-cli` directly.

**A merge is handed what it may touch, instead of reaching for it.**

`weave_core::host::Host` is new: a duplicate-name threshold and an optional
line-level merge, built at a program's entry point and passed down. Two things
inside the merge were not functions of their inputs — `WEAVE_MAX_DUPLICATES`
was read in the middle of the decision that used it, and the line-level route
spawned `git merge-file` with three temporary files. Both are worth having;
neither could be declined.

`entity_merge_fmt` and `entity_merge_with_registry` take a `&Host`, as does
`explain::explain`. `entity_merge` keeps its signature and runs against
`Host::default()`, which grants nothing — so the four-argument call is now a
function of its three inputs. Callers wanting the previous behavior pass:

```rust
let host = weave_core::host::Host {
    line_merge: Some(weave_core::host::git_line_merge),
    ..Default::default()
};
```

`WEAVE_MAX_DUPLICATES` still works: `weave-driver` reads it and puts it on the
host. It is documented in `weave-driver --help` for the first time.

**`weave_core::git` returns a typed error.** All six functions returned
`Box<dyn std::error::Error>` built from `format!`, so "git is not installed",
"this is not a repository", "these two refs share no history" and "git
declined" were one type. They are now `GitError::{NotRunnable, NotARepository,
NoMergeBase, Refused}`, each carrying its operands. Code using `?` into
`Box<dyn Error>` still compiles.

**`weave_core::stats` takes the path it reads and writes.** `load()` and
`save()` derived `~/.weave/stats.json` from the environment themselves.
`load(&Path)` and `save(&Path) -> bool` take it; `stats::default_path(home)`
offers the conventional location to a caller that wants it. `save` reports
whether the write landed instead of swallowing it.

**`weave_cli::patch` types its two boundaries.** `PatchOp::op` is now the `Op`
enum — the schema's own alphabet — rather than a `String` that let an
unrecognised verb decode and be silently ignored. `patch::apply` returns
`PatchError` instead of a sentence with two hashes in it, and
`patch::parse_ops_doc` is the only route from bytes to an `OpsDoc`, refusing
unknown fields and unknown majors by name.

### Stricter — documents that did not match their own schemas

Both published schemas declare `additionalProperties: false`; no decoder
enforced it. Now they do. An ops document with a misspelled field, or an MCP
tool call with a misspelled argument, is refused instead of silently accepted
with the field discarded. A `weave_check` call with nothing but unrecognised
keys used to answer confidently about revisions the caller never named.

### Fixed

- The MCP server answers `invalid_params` for a caller's own mistake — an
  unknown entity name, an unreadable path, no repository — instead of
  `internal_error` for everything, which told an agent "stop asking" when the
  right answer was "ask differently".
- The GitHub webhook decodes the event payload into a type. A missing field
  and a hostile one used to produce the same empty string, and the request
  returned 200 having read nothing.

## 0.4.0

### Breaking — library API

If you use weave as a CLI, a git merge driver, or through the MCP server,
nothing here affects you. These changes affect Rust code that depends on the
`weave-core` or `weave-crdt` crates directly.

The two library crates published a much larger surface than they supported.
Most of it was reachable by accident rather than on purpose, and some of it
had two spellings for the same function. This release cuts the surface down to
what is actually meant to be called, which breaks code that reached past it.

**`weave-crdt`: the module paths are gone.**

Every module (`content`, `error`, `merge`, `ops`, `state`, `sync`) is now
private to the crate. The `pub use` list in `lib.rs` is the entire public
surface. Previously each item had two paths — `weave_crdt::update_entity_content`
and `weave_crdt::content::update_entity_content` reached the same function —
and the two were free to drift apart without any caller noticing.

Migration: drop the module segment. `weave_crdt::sync::reconstruct_file_from_crdt`
becomes `weave_crdt::reconstruct_file_from_crdt`. Everything that was reachable
through a module path and is still supported is re-exported from the crate
root under the same name.

**`weave-crdt::record_modification` is removed.** It was a strictly weaker
duplicate of `update_entity_content`: same vector-clock increment, same three
summary writes, but it never wrote the `writes` register entry, so it could
leave `content_hash` naming a write no replica could join. Use
`update_entity_content`, which takes the content alongside the hash.

**`weave-crdt::MergeState` is no longer exported.** It was part of no supported
flow. `CrdtMergeResult` and `VersionVector` are unchanged.

**`weave-core::reconstruct` is removed.** The v1 reconstruct path it belonged
to no longer exists; the v2 pipeline does this work internally.

**`ResolutionStrategy::Fallback` is removed.** The variant was unconstructible —
`resolve` never emitted it, and a line-level fallback returns an empty audit
trail instead. If you matched on it exhaustively, that arm was dead. What is
true and now stated on `Op.fallback`: a fallback merge produces no ops at all,
so the read document is silent rather than flagged.

### Added — library API

- `weave-crdt`: `anchor_of`, `ordered_entity_ids` and `Anchor`, so a caller can
  read an entity's layout coordinate rather than infer it from the order.
- `weave-crdt`: `join`, `apply_op`, `value_of`, `EntityOp`, `EntityValue` and
  `Write` — the join door, replacing ad-hoc detection.
- `weave-core`: the `binding`, `diagnose`, `explain`, `frame` and `v2` modules
  are public.

### Added — languages

`weave setup` now writes `merge=weave` lines for 17 more extensions:

    .kt  .tf  .hcl  .ml  .mli  .zig  .elm  .clj  .edn  .d
    .lua .fish .nix  .sql .tex  .pl   .csv

That is 38 languages and formats in total. The engine could already parse
these; `setup` simply had never claimed them, so git was handling them
line-by-line.

Each one earns its place by passing a five-scenario merge sweep
(`crates/weave-core/tests/language_coverage.rs`): two sides adding different
definitions merges clean, two sides rewriting the same definition conflicts,
a side that stood still is the identity, the same edit made twice lands once,
and nothing is dropped in any of them.

`.vue`, `.svelte`, `.erb` and `.hs` are **not** claimed, and the README no
longer says they are. weave can parse all four, but their entity model treats
a whole `<script>` block, template, or type signature as one unit, so two
people adding two different definitions conflict where they should merge, and
the conflict marker can land in the middle of a definition. Those files keep
getting git's line-level merge, which is the better answer until the parser
gains a real per-definition model for them.

### Fixed

- **npm package was unusable.** `package.json` declared `weave`,
  `weave-driver` and `weave-mcp` binaries, but `bin/` had been deleted from the
  tree, so every install produced commands pointing at files that were not
  there. All three wrappers are restored and the packed tarball is verified to
  contain them.
- `package.json` had been sitting at 0.3.4 while the crates were at 0.3.6.

### Changed

- Two merges that used to conflict now compose. When both sides edit inside one
  method body, weave resolves at the statement and expression level instead of
  handing back the whole method as a conflict — so one side adding a cache
  lookup while the other renames a call in the same return statement produces
  the composed result rather than a box. No edit is dropped either way; this
  turns some conflicts into clean merges, never the reverse.
- The in-tree test suite is 401 tests, up from 268. Three test files that had
  stopped shipping are back, and the language sweep is new.
- Documentation comments across the crates were reworded into plain
  engineering terms. The rules they describe are enforced by tests, and the
  comments now say that rather than borrowing a mathematical register for it.
  No behaviour changed.
