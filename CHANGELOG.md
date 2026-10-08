# Changelog

This file starts at 0.4.0. For earlier releases see the
[GitHub releases page](https://github.com/Ataraxy-Labs/weave/releases).

Versions are shared across every crate in the workspace and the npm package,
so `weave-core`, `weave-crdt`, `weave-driver`, `weave-cli`, `weave-mcp`,
`weave-github` and `@ataraxy-labs/weave` all move together.

## Unreleased

### Changed — seven commands, variations as flags

`weave --help` lists `setup`, `land`, `explain`, `check`, `preview`, `patch`
and `stats`, one line each saying which question the command answers.
`weave setup --off` replaces `unsetup`, `weave explain <file> --summary`
replaces `summary`, and `weave stats --bench` / `weave stats --repo <path>`
replace `bench` / `bench-repo`. The live-editing prototype (`claim`,
`release`, `status`, `apply`) moves under a hidden `weave experimental`
group. Every old name and flag still works with identical output; at a
terminal it prints a one-line note on stderr naming the new spelling, and
nothing extra in `--json` mode or when stdout is not a terminal.

### Changed — the MCP server lists one tool per command

`weave-mcp` lists five tools instead of 22: `weave_preview` (detail
`summary`, `findings` or `entities`, replacing `weave_preview_merge`,
`weave_findings` and `weave_merge_audit`), `weave_explain` (`markers=true`
replaces `weave_merge_summary`), `weave_check` (now also verifies the working
tree, like `weave check`), and the new `weave_land` and `weave_patch`. The
live-editing CRDT tools are listed only with `WEAVE_EXPERIMENTAL=1`; for
callers and blast radius use sem. Every old tool name still answers when
called (#179).

### Added — `weave land --check sem`

With `--onto` or `--queue`, `--check sem` (or `[land] check = sem` in
`.weave/config`) runs `sem check --base <tip> --json` on the exact tree to be
published, after `--verify-cmd` if both are given. Exit 0 lands; a failure, a
tree sem could not decide, unreadable output or no `sem` at all refuses and
publishes nothing. The sem check certificate is recorded with the gate
certificates and in the queue ticket's result. `--check none` turns a
configured default off for one run.

### Added — `weave land --queue` (experimental)

Many agents, one origin, one branch: `weave land --queue` submits HEAD to a
landing queue kept as append-only commit chains under `refs/weave/<branch>/`
in the shared remote and blocks until the candidate is landed or refused.
One lander at a time (a leased lock, `--lease-ttl`) merges each candidate onto
the tip in submission order, runs the gate and `--verify-cmd`, and
fast-forwards the branch, so no push can race another. Every ticket gets
exactly one result; a merge refusal leaves the merge in progress locally to
resolve in place. Only ref creations and fast-forwards are pushed, so origins
that deny non-fast-forwards and deletes work unchanged. The protocol may
change.

### Fixed — a merge may not state a `case` label or a map key twice

Two features that each added `case 3:` (different bodies) to one Go switch
merged clean and broke the build twice in a multi-agent run. The two
insertions are line-disjoint, so `git merge-file` merges them cleanly; the
entity merge's fallback took that answer; and `case 3:` is too short a line
for the line rules to count. `weave_core::verify` now reads keyed elements —
switch / match case labels (Go, JS/TS, Java, C/C++, Python `match`), Go
composite-literal keys, JS/TS object and Python dict keys, Rust struct-literal
fields — and refuses a merge that states a key more often in one container
than either side states it in any container (`keys:`). It is part of the
fail-closed check every composed merge passes, and also the last step of
`entity_merge` on every clean answer that is not one side's own file,
whichever rung wrote it (the large-file line route included). `weave check`
and `weave land`'s gate report it as `DUP`.

### Fixed — `weave land` examines files git merges line-cleanly

`weave land` counted files both sides changed that git merges line-cleanly as
"not examined". That is where the duplicate `case 3:` was. Such a file is now
checked: when the answer in the tree is git's merge, it must pass weave's own
merge check, or the file becomes a unit (REFUSED unless an answer passes the
gate).

### Added — `weave land` judges the answer already in the tree first

A file the merge driver resolved by itself (stage 0 in the index), or with
`--result <rev>` that revision's file, is labelled before any resolver is
asked: PROVEN when it is weave's certified merge, VERIFIED when it passes the
gate. Before, a driver-merged file the certificate could not prove went to
the resolver as if unresolved, and a resolver that only answers files it was
shown conflicted (the agent) declined, so `weave land` wrote conflict markers
into a file that had been clean. In one multi-agent run that was 28% of all
refused files. Replaying the `--onto` flow (driver, then `weave land` over the
whole merge) on real conflicting pairs, expr lands 36/54 conflicting pairs with no resolver
call instead of 31/54, 0 wrong.

### Added — `weave land --onto <remote>/<branch>` and `--verify-cmd`

The whole landing in one command: fetch, `git merge --no-commit` of the tip,
the gate over every file of the merge, commit, the gate over any other merge
on the branch not yet checked, `--verify-cmd` on the final tree, then a
fast-forward-only update of the remote branch. When the branch moved, the new
tip is merged and everything runs again; nothing is published that has not
passed the gate and the verify command against the exact tip it lands on.
Every refusal publishes nothing. A land script without these steps re-merged
a newer main after a push race and published it with no check on the final
tree; three of its broken mains were semantic conflicts no per-file gate can
see (two lexer changes that each compiled), which only a verify command that
runs the tests refuses. Setups should pass at least a build.

### Added — insertions into one collection inside one entity merge as a union

Two branches that each add an entry to the same registry map, a case to the
same switch, or a row to the same test table used to conflict whenever the
two additions landed on neighbouring lines, even though the entity merge had
already narrowed the conflict to one function. A new rung of the entity
ladder (`weave_core::elements`, tried after the container merge and before
the statement fold) merges such collections element by element: each side
must only insert, delete what the other left alone, or modify; insertions
both sides made at one point are ordered by key, so the answer is the same
from either direction.

By default it unites only collections whose meaning does not depend on
order: Go map and keyed struct literals, JS object literals and export lists,
Python dicts (constant or dotted-name keys), Go switches with a tag and no
`fallthrough`, JS switches whose cases cannot fall through, distinctly
named declarations (JS `function`, Go top-level `func`, Python `def`), Go `const`
blocks without `iota` or appended after their last constant, and slice /
array / list literals in test files. Ordered collections in product code,
runs of registry calls (`frame.bind("k", …)`), and `iota` insertions ahead of
an existing constant need the `weave-set` attribute. A key or case label
inserted twice, an element both sides changed that does not itself merge, a
type switch or tagless switch, and anything that does not reparse stay
conflicts. `WEAVE_DEBUG_ELEMENTS=1` says which guard refused.

### Added — `weave land` lands an element union without the resolver

weave's element union was clean, but `weave land`'s certificate did not
cover it, so every such file still went to the resolver. The certificate
(`weave-certify`) now has `elem_union`, its own check of an element union,
read from the four parse trees and sharing no code with the merge: every
element a side inserted is in the result verbatim, base elements keep base
order, no key, case label, constant or declared name is stated twice, a
point both sides inserted at is in a construct the (auto) policy treats as a
set, and every subtree parses. A file whose both-changed regions need it
(or `nest_eu`, `nest` over children it admits) lands VERIFIED, with rule
`elem_union` and no resolver call, after passing the same gate a resolver's
answer must pass. It is not PROVEN: the PROVEN rule set is
the fixed proof set. Collections opted in with `weave-set` are not covered by
the check and still go to the resolver. The report gains `union_allowances`.

The same check also reads a whole file as one statement list when the
region-by-region verdict fails: both sides appending top-level declarations
at one point (Go package-scope `func`/`type`/methods, JS `function`, Python
`def`, distinct names) leaves the text after the first new entity a region
neither side wrote, which the per-region certificate cannot admit. Such a
file lands VERIFIED (`elem_union (whole file)`) under the same policy and
gate; appended classes or other statements still go to the resolver.

### Changed — a Go `iota` block is never silently renumbered

Both sides inserting constants ahead of an existing one in an `iota` block
gave that constant a value neither side wrote (`OpEnd` at `n+3`, where ours
had `n+2` and theirs `n+1`), and weave merged it clean. The merge gate now
refuses that (check `iota`) unless the file's `weave-set` attribute names the
block by its type or first constant; appending after the last constant is
unaffected.

### Fixed — `weave land` no longer verifies a one-sided resolution

The gate behind `weave land --resolver` labelled VERIFIED an answer that kept
only one side of a conflict block, silently dropping the other side's change
(seen on two adjacent entries added to one dict literal). A new gate rule,
`DROPPED`, holds every conflict block of git's diff3 merge to both sides'
changes relative to base, measured in tokens: what one side added there must
survive (`DROPPED`), and what one side deleted there must not come back
(`UNDELETED`). Identical changes and one side subsuming the other still pass;
an in-block delete-vs-modify is refused whichever side is kept.

### Changed — the merge driver fails closed

When weave cannot justify a composition, it now reports a conflict instead of
a clean merge. A clean exit (0) means the result passed every check below; a
merge that fails one falls back to git's own line merge, and to a single
conflict box if even that comes back clean but unverifiable.

- **Checked before a merge is called clean** (`weave_core::verify`): no conflict
  marker in the output; every line both sides kept is present and no line is
  stated more often than applying both sides' edits states it (a deleted import
  no longer comes back); no name, member or `package` declared more often than
  either side declares it (no duplicate imports, no `const` redeclaring an
  import, no second `package` line, no doubled enum entry); no name used that an
  input imported or declared at file scope and the result no longer binds —
  read in any reference position (a bare argument, a type argument), with
  imports read per language (Java, Kotlin, Go, Rust, C#, Dart as well as
  JavaScript and Python), and `weave check`'s `DANGLING` applies the same rule
  to a resolution; the result parses whenever
  both sides do; in prose (Markdown, reStructuredText, text, LaTeX) two sides
  rewriting the same passage differently is a conflict, not a section-by-section
  interleave. A JSON, TOML or YAML result must still load whenever either side
  does, and may not state a key at one table path more often than either side
  does — including when one side's file is taken whole because it already
  carries the other side's edit. The refusal is named on the conflict
  (`unverified merge`).
- **Two edits to one body compose only at line granularity**, as in git: the
  statement fold no longer answers clean, and the token-level expression fold is
  gone. Two edits to one line, or to adjacent lines of one function, conflict.
- **The import-region union** applies only when both sides changed nothing but
  import lines in the region and the union binds no name twice.
- **The line-level route is git's merge.** It no longer splits lines at `{ } ;`
  to merge the pieces when git's merge conflicts.

Expect fewer clean merges than 0.5.4 and no clean merge that is wrong in the
ways above.

### New — conflicts whose answer does not depend on intent

A merge is right for every intent exactly where the two edits commute. When the
entity merge refuses a file, weave now tries the rules below on each region
git's line merge left in conflict; if every region falls under one, the file is
settled, checked by the same gate as any composition, and the driver says which
rule (`weave: <file> settled by rule: …`). Otherwise it stays conflicted.

- **Agreement / subsumption (D0, D1)** — both sides wrote the same lines, or one
  side's change to the region contains the other's. Blank lines are layout;
  alignment is global; a line the smaller edit deleted that the larger side
  still states was moved, not deleted; a deletion is carried only by a deletion
  (one side removing a statement the other rewrote stays a conflict); never
  into a side whose file does not parse when the other's does.
- **Set union (D3)** — regions made only of import lines, or of the lines of an
  ignore / requirements / `go.sum` file, merge as a set. An import's name list
  from one module merges name by name (a replaced import is a replacement); no
  local name may be bound by two different import lines; an item both sides
  changed differently, or a side stating one item twice, stays a conflict.
- **Layout-only side (D6)** and **layout-equal creations (D4)** — a side whose
  change is layout only (whitespace, line breaks, a byte-order mark — comments
  are content; judged on the syntax tree where there is a grammar) yields the
  other side's text, for the whole file or one region in its context.
- Never settled: modify/delete, two renames of one declaration, inputs that
  already carry markers, and any file where one side deleted a declaration the
  other changed in more than layout. Choices between equivalent texts do not
  depend on which side is called ours.

### Fixed

- A side that moved a declaration it edited could silently win against the
  other side's deletion of it: the subsumption rule read the move as the same
  deletion. It is a modify/delete conflict again (line and declaration guards).
- Re-merging an output against the same side grew the file by a blank line per
  pass when the side's gap had lost its neighbour; a blank run with nothing
  before it (or after it, rolled past the last declaration) is no longer
  emitted, so re-merge is a fixpoint.
- The merge gate no longer counts the no-grammar fallback's line-range
  `chunk`s as declarations.
- `weave check` could hang forever reading merge stages through
  `git cat-file --batch` on merges that touched many files (a two-pipe
  deadlock). Requests are now written on their own thread.
- `weave check` sat idle for hours in a partial (blobless) clone: `cat-file`
  fetched each missing blob in its own network round trip. Stages are now read
  with lazy fetching off and missing blobs fetched in one batch; a blob that
  still cannot be read makes that one file `UNREAD` instead of ending the whole
  check with no verdicts. The old read also mistook a non-local blob for a file
  the revision did not have.
- `weave check` spun for minutes on large merges: files over 1 MB (the merge's
  own structure ceiling, now `weave_core::merge::STRUCTURE_LIMIT_BYTES`) are no
  longer parsed for structure — sem-core's entity-id disambiguation is
  quadratic in same-named siblings — and the repo-wide dangling pass parses each
  file once instead of once per subject.
- `weave check` never waits on git without a deadline, and `--timeout`
  (default 300 s) ends any run that outlives it with NOTHING WAS VERIFIED and
  exit 2. Any error also exits 2; exit 1 keeps meaning findings.
- `weave check` prints a verdict line for every file the merge touched,
  including a file a side deleted as asked and a symlink or submodule.
- `weave check` during a rebase or cherry-pick uses the replayed commit's parent
  as the base (#157).
- `weave check` judges a modify/delete by what each resolution can lose, under
  its own class `MODDEL` (it was reported as `MARKERS`, and every deletion was a
  finding while every keep passed). Deleting the file is a finding only if a
  surviving file still calls a name only it defined, or the modifier's edit is
  neither layout-only (whitespace; comments count) nor already in the file the
  deleter moved the content to. When that successor exists and the edit
  re-applies to it cleanly, the finding carries the ported edit as a patch.
  Keeping the file is a finding only if the successor now defines the same
  names (a moved file kept twice); a kept file calling a name the deleter
  removed is `DANGLING`, as before. Every other modify/delete resolution is
  `review (MODDEL)` — an advisory, exit 0: whether the file should exist is the
  authors' call. A missing file that is not a modify/delete is `MISSING`.
  The move-plus-edit port lives here rather than in the driver because git
  never gives a merge driver a modify/delete, nor two paths at once; it is
  offered, not applied — `weave check` does not write the working tree.
- `weave check` reads a file git relocated into a renamed directory
  (`CONFLICT (file location)`) against the side that added it, at the path it
  was added at. It was checked against no stages at all — every line it states
  twice was a `DUP` — and the path it was added at was reported missing.

## 0.5.4

### Fixed

- A merged file could silently drop top-level text (statements, comments, strings) that every version agreed on, when one side deleted every entity in the file while another entity elsewhere in the same file still survived the merge. `extract_regions` keys a side with zero entities as a single `file_only` region, while the other sides key that same trailing text as `file_header`/`file_footer`/`between:...`; the interstitial 3-way merge worked per-key, so on the zero-entity side the `file_footer` key simply wasn't present and read back as `""` — and when the other two sides agreed on `file_footer`, the merge ladder's `base == theirs -> take ours` rung picked that empty string, discarding text no version had touched. `render` now folds the correctly-computed `file_only` text into the same trailing join as `file_footer` unconditionally, instead of only when the whole document merged to zero surviving entities. Fixes #148. (#162)

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
