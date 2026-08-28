//! The merge's front door, its result types, and the machinery around the
//! decision itself.
//!
//! The decision itself lives in [`crate::v2`]: match, classify, plan, resolve,
//! render. [`entity_merge_fmt`] is the entry point every
//! caller uses, and its job is to hand v2 three texts and a marker format and
//! then get out of the way.
//!
//! What is left here is everything around that decision, and it falls into
//! three groups:
//!
//!   1. **The pre-checks that can refuse before v2 runs at all** — binary
//!      content, a file over 1MB, inputs that already carry conflict markers.
//!   2. **Helpers v2 calls into** — interstitial and import merging, decorator
//!      handling, container wrapper extraction, the scoped marker writer. These
//!      predate v2 and were kept because v2 needed exactly them; each is reached
//!      from `v2::mod`, `v2::resolve`, `statement` or `container`.
//!   3. **The line-level route** — [`line_level_fallback`] and what it calls
//!      (`skip_expansion`, `expand_separators`, `git_merge_file`, `diffy_fallback`).
//!      Reached only after v2 returns a typed `Unsupported` verdict, or on the
//!      size and binary pre-checks above. Nothing on this route produces an
//!      audit trail, so a fallback merge reports bytes and no per-entity story.
//!
//! There is no fourth group. A helper here that no live path reaches is a bug,
//! not history.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

use sem_core::model::entity::SemanticEntity;
use sem_core::parser::plugins::create_default_registry;
use sem_core::parser::registry::ParserRegistry;
use serde::Serialize;

/// Static parser registry shared across all merge operations.
/// Avoids recreating 11 tree-sitter language parsers per merge call.
pub(crate) static PARSER_REGISTRY: LazyLock<ParserRegistry> =
    LazyLock::new(create_default_registry);

/// Extensions that PARSE but merge worse than git's own line strategy, so
/// `weave setup` must not claim them for `merge=weave`. Their entity model
/// boxes disjoint additions and cuts the marker mid-definition; the observed
/// output for each is recorded in `weave-core/tests/language_coverage.rs`.
///
/// This is the *only* subtraction applied to the registry's supported set —
/// everything else the parser recognises is entity-merged. An extension earns a
/// place here by failing the coverage sweep, not by taste. The whole `.svelte*`
/// family is listed because every one of those compound suffixes routes to the
/// Svelte plugin, which merges the same way; the parity guard in
/// `weave-core/tests/setup_extension_coverage.rs` fails if the registry ever
/// grows a Svelte (or Vue/ERB/Haskell) suffix this list forgot.
pub const DECLINED_EXTENSIONS: &[&str] = &[
    ".hs",
    ".vue",
    ".erb",
    ".svelte",
    ".svelte.js",
    ".svelte.ts",
    ".svelte.test.js",
    ".svelte.test.ts",
    ".svelte.spec.js",
    ".svelte.spec.ts",
];

/// The file extensions `weave setup` should write `*.<ext> merge=weave` lines
/// for: every extension the parser registry recognises in this build, minus
/// [`DECLINED_EXTENSIONS`]. Each entry keeps its leading dot (e.g. `".ts"`) and
/// the result is sorted and deduplicated.
///
/// This is the single source of truth for setup's language coverage. Because it
/// reads the live [`ParserRegistry`], adding a grammar to `sem-core` (a new
/// `LanguageConfig`, or `.mts`/`.cts` on an existing one) extends what setup
/// claims automatically — the two can no longer drift. `weave-cli`'s setup
/// command consumes this; `weave-core/tests/setup_extension_coverage.rs` guards
/// the parity.
pub fn supported_merge_extensions() -> Vec<String> {
    let declined: HashSet<&str> = DECLINED_EXTENSIONS.iter().copied().collect();
    let mut exts: Vec<String> = PARSER_REGISTRY
        .registered_extensions()
        .into_iter()
        .filter(|ext| !declined.contains(ext))
        .map(str::to_string)
        .collect();
    exts.sort_unstable();
    exts.dedup();
    exts
}

use crate::conflict::{classify_conflict, ConflictKind, EntityConflict, MarkerFormat, MergeStats};
use crate::host::{Host, LineMergeStyle};
use crate::region::FileRegion;
use crate::validate::SemanticWarning;

/// How an individual entity was resolved during merge.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStrategy {
    Unchanged,
    OursOnly,
    TheirsOnly,
    ContentEqual,
    DiffyMerged,
    DecoratorMerged,
    InnerMerged,
    /// The body was a sequence of statements and the statement triples merged
    /// (`statement.rs`). Reached only after every other clean path refused.
    StatementMerged,
    /// A conflict the whole ladder refused, resolved by `v2::bind` under
    /// disjoint composition: the two sides inserted into one slot and neither block
    /// writes what the other writes or reads. Every one of these
    /// carries a `composition_licensed` finding with the evidence.
    FootprintLicensed,
    ConflictBothModified,
    /// A conflict the statement fold scoped down to the statements that
    /// actually disagree, inside a body diff3 refused whole. Same
    /// verdict as `ConflictBothModified`, a much smaller marked region.
    ConflictStatementScoped,
    ConflictModifyDelete,
    ConflictBothAdded,
    ConflictRenameRename,
    ConflictRenameModify,
    AddedOurs,
    AddedTheirs,
    Deleted,
    Renamed {
        from: String,
        to: String,
    },
}

impl ResolutionStrategy {
    /// Which rung of the ladder refused, as a stable name — `None` when this
    /// strategy resolved and nothing refused.
    ///
    /// `kind: "both_modified"` is a VERDICT: it says the two sides disagreed,
    /// which the reader could see from the marker. This says *which guard*
    /// declined, and the guards are not interchangeable: `statement_fold` means
    /// diff3, the decorator merge and the container merge all passed and two
    /// edits landed in one statement; `merge_ladder_exhausted` means every rung
    /// refused. Naming the guard is the one part of weave's output that gets
    /// used unprompted, which is why it moved from a JSON document nobody
    /// opened into the marker everybody has to read.
    ///
    /// One owner: `weave-mcp`'s findings producer used to carry its own copy of
    /// this table, and the marker renderer had none. Exhaustive, so a new
    /// refusal cannot compile until it has a name here.
    pub fn guard(&self) -> Option<&'static str> {
        match self {
            ResolutionStrategy::ConflictBothModified => Some("merge_ladder_exhausted"),
            ResolutionStrategy::ConflictStatementScoped => Some("statement_fold"),
            ResolutionStrategy::InnerMerged => Some("container_member"),
            ResolutionStrategy::ConflictModifyDelete => Some("modify_delete_guard"),
            ResolutionStrategy::ConflictBothAdded => Some("both_added_divergent"),
            ResolutionStrategy::ConflictRenameRename => Some("rename_rename_divergent"),
            ResolutionStrategy::ConflictRenameModify => Some("rename_vs_edit_guard"),
            _ => None,
        }
    }

    /// The guard name a marker must print. A conflict always has a refusing
    /// guard; `guard()` returning `None` on a conflicting strategy would be a
    /// bug, and naming the fallback here keeps the renderer total without
    /// letting it invent a name.
    pub(crate) fn guard_or_ladder(&self) -> &'static str {
        self.guard().unwrap_or("merge_ladder_exhausted")
    }
}

/// Audit record for a single entity's merge resolution.
#[derive(Debug, Clone, Serialize)]
pub struct EntityAudit {
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub resolution: ResolutionStrategy,
}

/// Result of a merge operation.
#[derive(Debug)]
pub struct MergeResult {
    pub content: String,
    pub conflicts: Vec<EntityConflict>,
    pub warnings: Vec<SemanticWarning>,
    pub stats: MergeStats,
    pub audit: Vec<EntityAudit>,
}

impl MergeResult {
    /// Whether the merge conflicted — a question about the *decisions*, not
    /// about the bytes.
    ///
    /// This used to also scan the output for `<<<<<<< ours`, because
    /// reconstruction could embed markers the conflict list did not know about
    /// (an interstitial merged by diff3, a scoped inner conflict). That is a
    /// symptom of two owners for one fact. In the v2 pipeline every conflict is
    /// a `Disposition::Conflict` before it is ever text, so the typed record is
    /// the only source of truth, and a clean merge can no longer report itself
    /// dirty because its *output* happens to contain the marker string.
    ///
    /// One marker scan survives, and it is not this one: `entity_merge*`
    /// refuses outright when an **input** already contains markers
    /// ([`has_conflict_markers`]), so a file that legitimately quotes
    /// `<<<<<<<` — a test fixture, a merge-tool's own documentation — is still
    /// declared conflicted before the entity model is ever consulted. That is a
    /// deliberate refusal to merge a base that is not a program, not a verdict
    /// about entities; it is a known, documented limit rather than a silent
    /// one.
    pub fn is_clean(&self) -> bool {
        self.conflicts.is_empty()
    }
}

/// Perform entity-level 3-way merge.
///
/// Takes the line-level route on the typed `Unsupported` verdicts raised by
/// `v2::mod` — no parser for the file type, zero entities out of non-empty
/// content, both sides creating the file, or identity too ambiguous to key on
/// (`has_excessive_duplicates`) — and, before any of that, on a file over 1MB
/// or one that is binary. The verdict is a value, not a bare `None`, so the
/// route a file took is answerable after the fact.
/// Runs against [`Host::default`] — no subprocess, no temporary files, no
/// environment read. Callers that want the `git merge-file` second opinion
/// grant it explicitly through [`entity_merge_fmt`].
pub fn entity_merge(base: &str, ours: &str, theirs: &str, file_path: &str) -> MergeResult {
    entity_merge_fmt(
        base,
        ours,
        theirs,
        file_path,
        &MarkerFormat::default(),
        &Host::default(),
    )
}

/// Perform entity-level 3-way merge with configurable marker format.
///
/// There is no longer a watchdog thread here. The timeout existed because
/// rename detection compared candidate pairs, so a file full of same-shaped
/// entities made the merge quadratic and a large input could outrun any
/// deadline. v2's matcher generates candidates from indexes —
/// body-hash buckets paired in key order, a prefix-filtered token index, and a
/// rare-token index for short bodies — so there is no pairwise scan left to
/// blow up, and the bound is pinned by a test rather than assumed. Racing a thread against a
/// clock was a way of not knowing the complexity; knowing it is better.
pub fn entity_merge_fmt(
    base: &str,
    ours: &str,
    theirs: &str,
    file_path: &str,
    marker_format: &MarkerFormat,
    host: &Host,
) -> MergeResult {
    entity_merge_with_registry(
        base,
        ours,
        theirs,
        file_path,
        &PARSER_REGISTRY,
        marker_format,
        host,
    )
}

pub fn entity_merge_with_registry(
    base: &str,
    ours: &str,
    theirs: &str,
    file_path: &str,
    registry: &ParserRegistry,
    marker_format: &MarkerFormat,
    host: &Host,
) -> MergeResult {
    // The `refused_by:` line's comment prefix is a fact about the file, and
    // this is the one function that has both the file path and the marker
    // format. Deriving it here means every consumer gets a refusal line that is
    // a comment in the language it lands in, and no consumer has to know that.
    let marker_format = &marker_format.clone().for_file(file_path);

    // The agreement axiom, ahead of every guard below it: when the two sides
    // are byte-identical there is nothing to merge, so no reason to refuse.
    // Behind the marker guard this axiom was defeated by an ancestor that
    // merely QUOTES marker syntax — a fixture, or documentation about merging —
    // which conflicted a merge both sides had already resolved identically and
    // proposed the stale ancestor's bytes as the resolution. The refusal is a
    // domain restriction on merging; an axiom that needs no base outranks it.
    if ours == theirs {
        return MergeResult {
            content: ours.to_string(),
            conflicts: vec![],
            warnings: vec![],
            stats: MergeStats::default(),
            audit: vec![],
        };
    }

    // Guard: if any input already contains conflict markers (e.g. AU/AA conflicts
    // where git bakes markers into stage blobs), report as conflict immediately.
    // We can't do a meaningful 3-way merge on pre-conflicted content.
    if has_conflict_markers(base) || has_conflict_markers(ours) || has_conflict_markers(theirs) {
        let mut stats = MergeStats::default();
        stats.entities_conflicted = 1;
        stats.mark_fallback();
        // Use whichever input has markers as the merged content (preserves
        // the conflict for the user to resolve manually).
        let content = if has_conflict_markers(ours) {
            ours
        } else if has_conflict_markers(theirs) {
            theirs
        } else {
            base
        };
        let complexity = classify_conflict(Some(base), Some(ours), Some(theirs));
        return MergeResult {
            content: content.to_string(),
            conflicts: vec![EntityConflict {
                entity_name: "(file)".to_string(),
                entity_type: "file".to_string(),
                kind: ConflictKind::BothModified,
                complexity,
                ours_content: Some(ours.to_string()),
                theirs_content: Some(theirs.to_string()),
                base_content: Some(base.to_string()),
            }],
            warnings: vec![],
            stats,
            audit: vec![],
        };
    }

    // Fast path: if base == ours, take theirs entirely
    if base == ours {
        return MergeResult {
            content: theirs.to_string(),
            conflicts: vec![],
            warnings: vec![],
            stats: MergeStats {
                entities_theirs_only: 1,
                ..Default::default()
            },
            audit: vec![],
        };
    }

    // Fast path: if base == theirs, take ours entirely
    if base == theirs {
        return MergeResult {
            content: ours.to_string(),
            conflicts: vec![],
            warnings: vec![],
            stats: MergeStats {
                entities_ours_only: 1,
                ..Default::default()
            },
            audit: vec![],
        };
    }

    // The subsumption rule: the unit fast path above, with "changed nothing"
    // weakened to "wrote nothing the other side did not write". If every hunk
    // one side wrote is carried by a hunk the other side wrote — same base
    // lines gone, same lines written, in order — then that other side's file
    // already has both edits in it, and it is a file a developer wrote rather
    // than one we composed. See `subsumption.rs` for what "carried" excludes.
    if !is_binary(base) && !is_binary(ours) && !is_binary(theirs) {
        if let Some(side) = crate::subsumption::subsuming_side(base, ours, theirs) {
            let (content, stats) = match side {
                crate::subsumption::Superset::Ours => (
                    ours,
                    MergeStats {
                        entities_ours_only: 1,
                        ..Default::default()
                    },
                ),
                crate::subsumption::Superset::Theirs => (
                    theirs,
                    MergeStats {
                        entities_theirs_only: 1,
                        ..Default::default()
                    },
                ),
            };
            return MergeResult {
                content: content.to_string(),
                conflicts: vec![],
                warnings: vec![],
                stats,
                audit: vec![],
            };
        }
    }

    // Binary file detection: if any version has null bytes, use git merge-file directly
    if is_binary(base) || is_binary(ours) || is_binary(theirs) {
        let mut stats = MergeStats::default();
        stats.mark_fallback();
        return line_merge_file(base, ours, theirs, stats, host);
    }

    // Large file fallback
    if base.len() > 1_000_000 || ours.len() > 1_000_000 || theirs.len() > 1_000_000 {
        return line_level_fallback(base, ours, theirs, file_path, host);
    }

    // ------------------------------------------------------------------
    // v2: the typed pipeline (Match -> Classify -> Resolve -> Bind -> Plan
    // -> Render). It owns every file the entity model actually describes.
    // The line-level path below is reached only for the constructs v2
    // reports as outside that model — no grammar, no entities, both sides
    // creating a structured file from nothing, or identity so ambiguous
    // that per-entity verdicts would be fiction. Those are properties of the
    // input, not fallbacks from v2 failing.
    // ------------------------------------------------------------------
    match crate::v2::merge_file(base, ours, theirs, file_path, registry, marker_format, host) {
        Ok(result) => result,
        Err(_unsupported) => line_level_fallback(base, ours, theirs, file_path, host),
    }
}

pub(crate) fn is_whitespace_only_diff(a: &str, b: &str) -> bool {
    if a == b {
        return true; // identical, not really a "whitespace-only diff" but safe
    }
    let a_normalized: Vec<&str> = a
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    let b_normalized: Vec<&str> = b
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    a_normalized == b_normalized
}

/// A blank-line count, merged the way a one-line region would be: a side that
/// left base's number alone has asserted nothing about it, so the other side's
/// number is the answer. If both moved it, the wider run wins — a blank line
/// neither version deleted is not this merge's to delete.
///
/// Every boundary the import rebuild re-synthesizes goes through here, so the
/// separators it writes are a function of the three inputs and not of a
/// constant somebody picked.
fn widest_gap(base: usize, ours: usize, theirs: usize) -> usize {
    if ours == base {
        theirs
    } else if theirs == base {
        ours
    } else {
        ours.max(theirs)
    }
}

/// Check if a line is a decorator or annotation.
/// Covers Python (@decorator), Java/TS (@Annotation), and comment-style annotations.
fn is_decorator_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('@')
        && !trimmed.starts_with("@param")
        && !trimmed.starts_with("@return")
        && !trimmed.starts_with("@type")
        && !trimmed.starts_with("@see")
}

/// Split content into (decorators, body) where decorators are leading @-prefixed lines.
fn split_decorators(content: &str) -> (Vec<&str>, &str) {
    let mut decorator_end = 0;
    let mut byte_offset = 0;
    for line in content.lines() {
        if is_decorator_line(line) || line.trim().is_empty() {
            decorator_end += 1;
            byte_offset += line.len() + 1; // +1 for newline
        } else {
            break;
        }
    }
    // Trim trailing empty lines from decorator section
    let lines: Vec<&str> = content.lines().collect();
    while decorator_end > 0
        && lines
            .get(decorator_end - 1)
            .is_some_and(|l| l.trim().is_empty())
    {
        byte_offset -= lines[decorator_end - 1].len() + 1;
        decorator_end -= 1;
    }
    let decorators: Vec<&str> = lines[..decorator_end]
        .iter()
        .filter(|l| is_decorator_line(l))
        .copied()
        .collect();
    let body = &content[byte_offset.min(content.len())..];
    (decorators, body)
}

/// Try decorator-aware merge: when both sides add different decorators/annotations,
/// merge them commutatively (like imports). Also try merging the bodies separately.
///
/// This handles the common pattern where one agent adds @cache and another adds @deprecated
/// to the same function — they should both be preserved.
pub(crate) fn try_decorator_aware_merge(
    base: &str,
    ours: &str,
    theirs: &str,
    decorators_compose: bool,
) -> Option<String> {
    let (base_decorators, base_body) = split_decorators(base);
    let (ours_decorators, ours_body) = split_decorators(ours);
    let (theirs_decorators, theirs_body) = split_decorators(theirs);

    // Only useful if at least one side has decorators
    if ours_decorators.is_empty() && theirs_decorators.is_empty() {
        return None;
    }

    // In languages where decorators COMPOSE (Python, TS/JS: application is
    // function composition, non-commutative), the stack order of two
    // one-sided additions is a semantic decision neither side made (e.g.
    // @cache outside @auth serves cached responses without an auth check).
    // Refuse to fabricate it: fall through to the conflict path so a
    // human/agent chooses the order. Java/C#/Kotlin annotations are
    // unordered metadata, so set-union remains correct there.
    if decorators_compose {
        let base_set_pre: HashSet<&str> = base_decorators.iter().copied().collect();
        let ours_new: Vec<&&str> = ours_decorators
            .iter()
            .filter(|d| !base_set_pre.contains(**d))
            .collect();
        let theirs_new: Vec<&&str> = theirs_decorators
            .iter()
            .filter(|d| !base_set_pre.contains(**d))
            .collect();
        if !ours_new.is_empty()
            && !theirs_new.is_empty()
            && ours_new.iter().any(|d| !theirs_new.iter().any(|t| t == d))
        {
            return None;
        }
    }

    // Merge bodies using diffy (or take unchanged side)
    let merged_body = if base_body == ours_body && base_body == theirs_body {
        base_body.to_string()
    } else if base_body == ours_body {
        theirs_body.to_string()
    } else if base_body == theirs_body {
        ours_body.to_string()
    } else {
        // Both changed body — try diffy on just the body
        diffy_merge(base_body, ours_body, theirs_body)?
    };

    // Merge decorators commutatively (set union)
    let base_set: HashSet<&str> = base_decorators.iter().copied().collect();
    let ours_set: HashSet<&str> = ours_decorators.iter().copied().collect();
    let theirs_set: HashSet<&str> = theirs_decorators.iter().copied().collect();

    // Deletions
    let ours_deleted: HashSet<&str> = base_set.difference(&ours_set).copied().collect();
    let theirs_deleted: HashSet<&str> = base_set.difference(&theirs_set).copied().collect();

    // Start with base decorators, remove deletions
    let mut merged_decorators: Vec<&str> = base_decorators
        .iter()
        .filter(|d| !ours_deleted.contains(**d) && !theirs_deleted.contains(**d))
        .copied()
        .collect();

    // Add new decorators from ours (not in base)
    for d in &ours_decorators {
        if !base_set.contains(d) && !merged_decorators.contains(d) {
            merged_decorators.push(d);
        }
    }
    // Add new decorators from theirs (not in base, not already added)
    for d in &theirs_decorators {
        if !base_set.contains(d) && !merged_decorators.contains(d) {
            merged_decorators.push(d);
        }
    }

    // Reconstruct
    let mut result = String::new();
    for d in &merged_decorators {
        result.push_str(d);
        result.push('\n');
    }
    result.push_str(&merged_body);

    Some(result)
}

/// Try 3-way merge on text using diffy. Returns None if there are conflicts.
pub(crate) fn diffy_merge(base: &str, ours: &str, theirs: &str) -> Option<String> {
    let result = diffy::merge(base, ours, theirs);
    result.ok()
}

/// The three files a `git merge-file` call has to be handed, and nothing else.
///
/// `git merge-file` takes three *paths*. There is no stdin form and no way to
/// pass three blobs on a command line, so any caller of it has to materialise
/// its inputs. What a caller does *not* have to do is build and tear down a
/// whole directory around them: both fallbacks used to `tempfile::tempdir()`
/// per call, paying a `mkdir` and an `rmdir` on top of the creates and unlinks
/// the files themselves need — and on top of the ~3.2ms `git` process, which
/// is the real cost and the one thing here that cannot be removed.
///
/// Merge interstitial regions from all three versions.
/// Uses set-based (order-independent) merge for import blocks.
/// Falls back to line-level 3-way merge for non-import content.
pub(crate) fn merge_interstitials(
    base_regions: &[FileRegion],
    ours_regions: &[FileRegion],
    theirs_regions: &[FileRegion],
    marker_format: &MarkerFormat,
) -> (HashMap<String, String>, Vec<EntityConflict>) {
    let base_map: HashMap<&str, &str> = base_regions
        .iter()
        .filter_map(|r| match r {
            FileRegion::Interstitial(i) => Some((i.position_key.as_str(), i.content.as_str())),
            _ => None,
        })
        .collect();

    let ours_map: HashMap<&str, &str> = ours_regions
        .iter()
        .filter_map(|r| match r {
            FileRegion::Interstitial(i) => Some((i.position_key.as_str(), i.content.as_str())),
            _ => None,
        })
        .collect();

    let theirs_map: HashMap<&str, &str> = theirs_regions
        .iter()
        .filter_map(|r| match r {
            FileRegion::Interstitial(i) => Some((i.position_key.as_str(), i.content.as_str())),
            _ => None,
        })
        .collect();

    // Sorted, not hashed: the loop below appends to `interstitial_conflicts`,
    // and a conflict list in hash order is a merge that is not a function of
    // its inputs. The per-key answers are independent, so the order only has
    // to be *some* fixed one.
    let mut all_keys: BTreeSet<&str> = BTreeSet::new();
    all_keys.extend(base_map.keys());
    all_keys.extend(ours_map.keys());
    all_keys.extend(theirs_map.keys());

    let mut merged: HashMap<String, String> = HashMap::new();
    let mut interstitial_conflicts: Vec<EntityConflict> = Vec::new();

    for key in all_keys {
        let base_content = base_map.get(key).copied().unwrap_or("");
        let ours_content = ours_map.get(key).copied().unwrap_or("");
        let theirs_content = theirs_map.get(key).copied().unwrap_or("");

        // If all same, no merge needed
        if ours_content == theirs_content {
            merged.insert(key.to_string(), ours_content.to_string());
        } else if base_content == ours_content {
            merged.insert(key.to_string(), theirs_content.to_string());
        } else if base_content == theirs_content {
            merged.insert(key.to_string(), ours_content.to_string());
        } else {
            // Both changed. Each rung of this ladder is a SPECIALISATION — a
            // reading of the region that lets two edits compose where diff3
            // would refuse. A specialisation is only allowed while it loses
            // nothing: the import union, for instance, rebuilds the region out
            // of its import lines, so a `//#region` banner or a comment
            // explaining an import is not in its vocabulary and used to vanish
            // silently. Each rung's answer is therefore checked before it is
            // accepted, and a rung that drops text all three versions agree on
            // is not taken.
            let keeps_everything = |text: &str| {
                !crate::container::drops_unanimous_lines(
                    base_content,
                    ours_content,
                    theirs_content,
                    text,
                )
            };
            // A line NEITHER version had before is an addition, and no rung is
            // allowed to answer by dropping one. `keeps_everything` cannot see
            // it — it asks about lines all three versions agree on — so an
            // import block with a `try: import pytz / except ImportError:`
            // guard interleaved in it came back with the guard gone: those
            // lines are neither imports (so the rebuild has no slot for them)
            // nor unanimous (so the no-loss backstop had no complaint). This
            // check has to account for every side's work, not only the part
            // both sides touched.
            let added: Vec<&str> = ours_content
                .lines()
                .chain(theirs_content.lines())
                .map(str::trim)
                .filter(|l| !l.is_empty() && !base_content.lines().any(|b| b.trim() == *l))
                .collect();
            let keeps_additions = |text: &str| {
                let have: HashSet<&str> = text.lines().map(str::trim).collect();
                added.iter().all(|l| have.contains(l))
            };
            let ws_ours = is_whitespace_only_diff(base_content, ours_content);
            let ws_theirs = is_whitespace_only_diff(base_content, theirs_content);
            let mut order_conflicted = false;
            // The rungs, in the order they are allowed to answer. The first
            // one whose answer loses nothing wins; `keeps_everything` is the
            // same predicate for all of them.
            let mut ladder: Vec<String> = Vec::new();
            if ws_ours && ws_theirs {
                // Both sides only changed whitespace; neither has content to
                // defer to, so one of them is taken outright.
                ladder.push(theirs_content.to_string());
            } else if is_import_region(base_content)
                || is_import_region(ours_content)
                || is_import_region(theirs_content)
            {
                // Order-preserving merge of the two import sequences. The
                // sides may give imports they SHARE contradictory relative
                // orders; module initialisation is side-effectful, so no
                // ordering is safe to invent and that is surfaced below.
                let (result, order_conflict) =
                    merge_imports_commutatively(base_content, ours_content, theirs_content);
                order_conflicted = order_conflict;
                if !order_conflict {
                    ladder.push(result);
                }
            }
            if !order_conflicted {
                if let Ok(text) = diffy::merge(base_content, ours_content, theirs_content) {
                    ladder.push(text);
                }
            }
            // The one-sided whitespace rung, DEMOTED below the line merge.
            // Taking the content-bearing side outright answers the whole
            // region, so a blank line the other side deleted at a declaration
            // boundary silently comes back. A whitespace-only edit is still an
            // edit; it only loses when the lines will not compose.
            if ws_ours != ws_theirs {
                ladder.push(
                    if ws_ours {
                        theirs_content
                    } else {
                        ours_content
                    }
                    .to_string(),
                );
            }
            let candidate = ladder
                .into_iter()
                .find(|text| keeps_everything(text) && keeps_additions(text));
            match candidate {
                Some(text) => {
                    merged.insert(key.to_string(), text);
                }
                None => {
                    let complexity = classify_conflict(
                        Some(base_content),
                        Some(ours_content),
                        Some(theirs_content),
                    );
                    let conflict = EntityConflict {
                        entity_name: key.to_string(),
                        entity_type: if order_conflicted {
                            "imports".to_string()
                        } else {
                            "interstitial".to_string()
                        },
                        kind: ConflictKind::BothModified,
                        complexity,
                        ours_content: Some(ours_content.to_string()),
                        theirs_content: Some(theirs_content.to_string()),
                        base_content: Some(base_content.to_string()),
                    };
                    merged.insert(
                        key.to_string(),
                        conflict.to_conflict_markers(marker_format, "merge_ladder_exhausted"),
                    );
                    interstitial_conflicts.push(conflict);
                }
            }
        }
    }

    (merged, interstitial_conflicts)
}

/// Check if a region is predominantly import/use statements.
/// Handles both single-line imports and multi-line import blocks
/// (e.g. `import { type a, type b } from "..."` spread across lines).
fn is_import_region(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return false;
    }
    let mut import_count = 0;
    let mut in_multiline_import = false;
    for line in &lines {
        if in_multiline_import {
            import_count += 1;
            let trimmed = line.trim();
            if trimmed.starts_with('}') || trimmed.ends_with(')') {
                in_multiline_import = false;
            }
        } else if is_import_line(line) {
            import_count += 1;
            let trimmed = line.trim();
            // Detect start of multi-line import: `import {` or `import (` without closing on same line
            if (trimmed.contains('{') && !trimmed.contains('}'))
                || (trimmed.starts_with("import (") && !trimmed.contains(')'))
                || (trimmed.starts_with("from ")
                    && trimmed.contains("import (")
                    && !trimmed.contains(')'))
            {
                in_multiline_import = true;
            }
        }
    }
    // If >50% of non-empty lines are imports, treat as import region
    import_count * 2 > lines.len()
}

/// Check if a line is a top-level import/use/require statement.
///
/// Only matches unindented lines to avoid picking up conditional imports
/// inside `if TYPE_CHECKING:` blocks or similar constructs.
fn is_import_line(line: &str) -> bool {
    // Skip indented lines: these are inside conditional blocks (TYPE_CHECKING, etc.)
    if line.starts_with(' ') || line.starts_with('\t') {
        return false;
    }
    let trimmed = line.trim();
    trimmed.starts_with("import ")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("use ")
        || trimmed.starts_with("require(")
        || trimmed.starts_with("const ") && trimmed.contains("require(")
        || trimmed.starts_with("package ")
        || trimmed.starts_with("#include ")
        || trimmed.starts_with("using ")
}

/// A complete import statement (possibly multi-line) as a single unit.
#[derive(Debug, Clone)]
struct ImportStatement {
    /// The full text of the import (may span multiple lines)
    lines: Vec<String>,
    /// The source module (e.g. "./foo", "react", "std::io")
    source: String,
    /// For multi-line imports: the individual specifiers (e.g. ["type a", "type b"])
    specifiers: Vec<String>,
    /// Whether this is a multi-line import block
    is_multiline: bool,
}

/// Extract specifiers from a single-line import statement.
/// e.g. "from X import A, B, C" → ["A", "B", "C"]
///      "import { A, B } from 'X'" → ["A", "B"]
///      "import X from 'Y'" → [] (default import, no named specifiers)
fn parse_single_line_specifiers(trimmed: &str) -> Vec<String> {
    // Python: "from X import A, B, C"
    if let Some(import_pos) = trimmed.find(" import ") {
        let after_import = &trimmed[import_pos + 8..];
        // Skip if it's "import *" or "import (..." (multi-line handled elsewhere)
        if after_import.starts_with('*') || after_import.starts_with('(') {
            return Vec::new();
        }
        return after_import
            .split(',')
            .map(|s| s.trim().trim_end_matches(';').to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    // JS/TS: "import { A, B } from 'X'"
    if trimmed.starts_with("import ") {
        if let Some(brace_start) = trimmed.find('{') {
            if let Some(brace_end) = trimmed.find('}') {
                let inner = &trimmed[brace_start + 1..brace_end];
                return inner
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
    }
    Vec::new()
}

/// Parse content into import statements, handling multi-line imports as single units.
fn parse_import_statements(content: &str) -> (Vec<ImportStatement>, Vec<String>) {
    let mut imports: Vec<ImportStatement> = Vec::new();
    let mut non_import_lines: Vec<String> = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if line.trim().is_empty() {
            non_import_lines.push(line.to_string());
            i += 1;
            continue;
        }

        if is_import_line(line) {
            let trimmed = line.trim();
            // Check for multi-line import: `import {` without `}` on same line
            let starts_multiline = (trimmed.contains('{') && !trimmed.contains('}'))
                || (trimmed.starts_with("import (") && !trimmed.contains(')'))
                || (trimmed.starts_with("from ")
                    && trimmed.contains("import (")
                    && !trimmed.contains(')'));

            if starts_multiline {
                let mut block_lines = vec![line.to_string()];
                let mut specifiers = Vec::new();
                let close_char = if trimmed.contains('{') { '}' } else { ')' };
                i += 1;

                // Collect lines until closing brace/paren
                while i < lines.len() {
                    let inner = lines[i];
                    block_lines.push(inner.to_string());
                    let inner_trimmed = inner.trim();

                    if inner_trimmed.starts_with(close_char) {
                        // This is the closing line (e.g. `} from "./foo"`)
                        break;
                    } else if !inner_trimmed.is_empty() {
                        // This is a specifier line — strip trailing comma
                        let spec = inner_trimmed.trim_end_matches(',').trim().to_string();
                        if !spec.is_empty() {
                            specifiers.push(spec);
                        }
                    }
                    i += 1;
                }

                let full_text = block_lines.join("\n");
                let source = import_source_prefix(&full_text).to_string();
                imports.push(ImportStatement {
                    lines: block_lines,
                    source,
                    specifiers,
                    is_multiline: true,
                });
            } else {
                // Single-line import — also parse specifiers for set-based merging
                let source = import_source_prefix(line).to_string();
                let specifiers = parse_single_line_specifiers(trimmed);
                imports.push(ImportStatement {
                    lines: vec![line.to_string()],
                    source,
                    specifiers,
                    is_multiline: false,
                });
            }
        } else {
            non_import_lines.push(line.to_string());
        }
        i += 1;
    }

    (imports, non_import_lines)
}

/// Order-preserving union of two import sequences.
///
/// Module imports execute top to bottom and their side effects are ordered, so
/// the two sides' sequences are sequences, not sets: the result must be a
/// linear extension of BOTH. Walks the two sequences together, taking shared
/// imports as anchors and slotting each side's own additions at the position
/// that side put them. Returns the merged sequence and whether the two sides
/// disagreed about the relative order of imports they SHARE — the one case no
/// linear extension exists for, which only a human can settle.
fn order_preserving_import_union<'a>(ours: &[&'a str], theirs: &[&'a str]) -> (Vec<&'a str>, bool) {
    let ours_set: HashSet<&str> = ours.iter().copied().collect();
    let theirs_set: HashSet<&str> = theirs.iter().copied().collect();
    let mut out: Vec<&'a str> = Vec::with_capacity(ours.len() + theirs.len());
    let mut emitted: HashSet<&str> = HashSet::new();
    let mut order_conflict = false;
    let (mut i, mut j) = (0usize, 0usize);

    while i < ours.len() || j < theirs.len() {
        if i < ours.len() && emitted.contains(ours[i]) {
            i += 1;
            continue;
        }
        if j < theirs.len() && emitted.contains(theirs[j]) {
            j += 1;
            continue;
        }
        let next = if i >= ours.len() {
            let l = theirs[j];
            j += 1;
            l
        } else if j >= theirs.len() {
            let l = ours[i];
            i += 1;
            l
        } else if ours[i] == theirs[j] {
            let l = ours[i];
            i += 1;
            j += 1;
            l
        } else if !theirs_set.contains(ours[i]) {
            // ours-only import: it belongs where ours put it
            let l = ours[i];
            i += 1;
            l
        } else if !ours_set.contains(theirs[j]) {
            // theirs-only import: it belongs where theirs put it
            let l = theirs[j];
            j += 1;
            l
        } else {
            // Both heads are shared imports, in opposite relative order. No
            // sequence satisfies both sides; report it and keep going so the
            // caller still has content to show alongside the markers.
            order_conflict = true;
            let l = ours[i];
            i += 1;
            l
        };
        if emitted.insert(next) {
            out.push(next);
        }
    }

    (out, order_conflict)
}

/// Merge import blocks, preserving each side's import ORDER and grouping.
///
/// Handles both single-line imports and multi-line import blocks.
/// For multi-line imports from the same source, merges specifiers as a set.
/// Returns the merged text and whether the two sides gave imports they share
/// contradictory relative orders (an honest conflict, not something to
/// silently pick a winner for).
fn merge_imports_commutatively(base: &str, ours: &str, theirs: &str) -> (String, bool) {
    let (base_imports, _) = parse_import_statements(base);
    let (ours_imports, _) = parse_import_statements(ours);
    let (theirs_imports, _) = parse_import_statements(theirs);

    let has_multiline = base_imports.iter().any(|i| i.is_multiline)
        || ours_imports.iter().any(|i| i.is_multiline)
        || theirs_imports.iter().any(|i| i.is_multiline);

    if has_multiline {
        return (
            merge_imports_with_multiline(
                base,
                ours,
                theirs,
                &base_imports,
                &ours_imports,
                &theirs_imports,
            ),
            false,
        );
    }

    // Single-line-only path.
    fn import_seq(content: &str) -> Vec<&str> {
        content.lines().filter(|l| is_import_line(l)).collect()
    }
    let base_seq = import_seq(base);
    let ours_seq = import_seq(ours);
    let theirs_seq = import_seq(theirs);
    let base_set: HashSet<&str> = base_seq.iter().copied().collect();
    let ours_set: HashSet<&str> = ours_seq.iter().copied().collect();
    let theirs_set: HashSet<&str> = theirs_seq.iter().copied().collect();

    // An import that was in base and is gone from one side was deleted there;
    // the deletion wins over the other side's untouched copy.
    let survives =
        |l: &&str| !(base_set.contains(*l) && (!ours_set.contains(*l) || !theirs_set.contains(*l)));
    let ours_live: Vec<&str> = ours_seq.iter().copied().filter(survives).collect();
    let theirs_live: Vec<&str> = theirs_seq.iter().copied().filter(survives).collect();

    // Who edited the ORDER? Restrict each side to the imports base also had:
    // if that subsequence still reads the way base read it, the side left the
    // ordering alone. A disagreement between the two sides is only a conflict
    // when BOTH of them moved shared imports — if one side is still at base's
    // order it has asserted nothing about ordering, so the other side's order
    // is the merge. Ours used to be the skeleton unconditionally, so a
    // one-sided "organize imports" collided with the untouched side and
    // produced a whole-block conflict where neither side actually disagreed.
    // Both views are taken over the same set — the base imports this side still
    // has AFTER the other side's deletions are applied — so a deletion cannot
    // masquerade as a reordering.
    let reordered = |live: &[&str]| -> bool {
        let live_set: HashSet<&str> = live.iter().copied().collect();
        let side_view: Vec<&str> = live
            .iter()
            .copied()
            .filter(|l| base_set.contains(l))
            .collect();
        let base_view: Vec<&str> = base_seq
            .iter()
            .copied()
            .filter(|l| live_set.contains(l))
            .collect();
        side_view != base_view
    };
    let ours_reordered = reordered(&ours_live);
    let theirs_reordered = reordered(&theirs_live);
    let theirs_leads = theirs_reordered && !ours_reordered;

    // The skeleton side supplies both the order and the blank-line grouping.
    let (skeleton, lead_live, follow_live) = if theirs_leads {
        (theirs, &theirs_live, &ours_live)
    } else {
        (ours, &ours_live, &theirs_live)
    };

    let (merged_seq, raw_order_conflict) = order_preserving_import_union(lead_live, follow_live);
    let order_conflict = raw_order_conflict && ours_reordered && theirs_reordered;

    // Grouping: blank-line-separated import groups are meaningful (stdlib vs.
    // local, etc.). The skeleton's grouping is the frame; an import only the
    // other side has inherits a group from its neighbours in the merged
    // sequence.
    let mut skeleton_group_of: HashMap<&str, usize> = HashMap::new();
    let mut group_count = 0usize;
    {
        let mut current = 0usize;
        let mut current_has_imports = false;
        for line in skeleton.lines() {
            if line.trim().is_empty() {
                if current_has_imports {
                    current += 1;
                    current_has_imports = false;
                }
            } else if is_import_line(line) {
                skeleton_group_of.insert(line, current);
                current_has_imports = true;
                group_count = group_count.max(current + 1);
            }
        }
    }
    let group_count = group_count.max(1);

    // An import the skeleton also has keeps the skeleton's group. One only the
    // other side has is bracketed by its nearest known neighbours in the
    // merged sequence and joins whichever of the two it shares more leading
    // source-path segments with. Two properties matter, and the old code had
    // neither: (1) the group lands *between* the neighbours' groups, so the
    // assignment is monotone along the sequence and emitting in sequence order
    // reproduces the union exactly; (2) it never consults a global prefix
    // table. The old `group_for` asked for an exact `import_source_prefix`
    // match and fell back to the LAST group, and `import_source_prefix` had no
    // Java arm at all — a bare `import a.b.C;` has no quotes, so every
    // Java import only one side added was declared prefix-unique and dumped at
    // the end. Then the emitter bucketed lines group by group and concatenated,
    // which threw away the union's ordering as well. That single miss silently
    // reordered or misplaced a large share of the imports either side added.
    let known: Vec<Option<usize>> = merged_seq
        .iter()
        .map(|l| skeleton_group_of.get(l).copied())
        .collect();
    let mut group_of_pos: Vec<usize> = Vec::with_capacity(merged_seq.len());
    for (idx, line) in merged_seq.iter().enumerate() {
        if let Some(g) = known[idx] {
            group_of_pos.push(g);
            continue;
        }
        let prev = (0..idx).rev().find(|&k| known[k].is_some());
        let next = (idx + 1..merged_seq.len()).find(|&k| known[k].is_some());
        let g = match (prev, next) {
            (None, None) => group_count - 1,
            (Some(p), None) => known[p].expect("prev is known"),
            (None, Some(n)) => known[n].expect("next is known"),
            (Some(p), Some(n)) => {
                let (gp, gn) = (
                    known[p].expect("prev is known"),
                    known[n].expect("next is known"),
                );
                if gp == gn
                    || import_prefix_affinity(line, merged_seq[n])
                        <= import_prefix_affinity(line, merged_seq[p])
                {
                    gp
                } else {
                    gn
                }
            }
        };
        group_of_pos.push(g);
    }

    // A `package` declaration is not an import. `is_import_line` lumps them
    // together — they share the "unindented top-of-file declaration" shape —
    // but no Java or Go file writes them in the same block, and when one side
    // *changes* the package the new declaration is a line the skeleton has
    // never seen, so it inherits the first import group and the blank line
    // between declaration and imports disappears. Keep the separation if any
    // of the three versions had it; none of them can be said to have asked for
    // it to go away.
    //
    // How WIDE that separation is, is also a fact about the file and not a
    // constant: a file that writes two blank lines between `package` and its
    // imports is not asking for one.
    let is_module_decl = |l: &str| l.trim_start().starts_with("package ");
    let declares_with_gap = |content: &str| -> usize {
        let lines: Vec<&str> = content.lines().collect();
        lines
            .iter()
            .position(|l| is_module_decl(l))
            .map(|i| {
                lines[i + 1..]
                    .iter()
                    .take_while(|l| l.trim().is_empty())
                    .count()
            })
            .unwrap_or(0)
    };
    let decl_gap = widest_gap(
        declares_with_gap(base),
        declares_with_gap(ours),
        declares_with_gap(theirs),
    );

    let mut import_lines: Vec<&str> = Vec::new();
    let mut prev_group: Option<usize> = None;
    for (idx, line) in merged_seq.iter().enumerate() {
        let group_changed = prev_group.is_some_and(|g| g != group_of_pos[idx]);
        let leaving_decl = idx > 0 && is_module_decl(merged_seq[idx - 1]) && !is_module_decl(line);
        if leaving_decl {
            import_lines.extend(std::iter::repeat_n("", decl_gap));
        } else if group_changed {
            import_lines.push("");
        }
        prev_group = Some(group_of_pos[idx]);
        import_lines.push(line);
    }

    let import_block = import_lines.join("\n");

    // Split non-import lines into prefix (before first import) and suffix
    // (after last import) to preserve file-leading directives like
    // `// @ts-nocheck`, shebangs, or license headers (fixes #94).
    // The blank lines INSIDE the header are content, not padding: a license
    // block, a blank, then `package foo;` is a shape a real Java file can have,
    // and gluing the two together is a visible edit nobody made. Only the
    // blank run that abuts the import block is dropped — that boundary is
    // re-synthesized below, so keeping it here would double it.
    //
    // "Abuts the import block" is one edge, not two. The prefix's OTHER edge is
    // the top of the region, and a blank line there is the file's own opening —
    // nothing re-synthesizes it, so trimming it deleted a line all three
    // versions wrote.
    let extract_prefix_suffix = |content: &str| -> (String, String) {
        fn trim_blank_edges<'s, 'a>(lines: &'s [&'a str]) -> &'s [&'a str] {
            let start = lines
                .iter()
                .position(|l| !l.trim().is_empty())
                .unwrap_or(lines.len());
            let end = lines
                .iter()
                .rposition(|l| !l.trim().is_empty())
                .map_or(start, |i| i + 1);
            &lines[start..end]
        }

        let all_lines: Vec<&str> = content.lines().collect();
        let first_import = all_lines.iter().position(|l| is_import_line(l));
        let last_import = all_lines.iter().rposition(|l| is_import_line(l));

        match (first_import, last_import) {
            (Some(first), Some(last)) => (
                trim_blank_edges(&all_lines[..first]).join("\n"),
                trim_blank_edges(&all_lines[last + 1..]).join("\n"),
            ),
            // No imports at all: the whole region is prefix.
            _ => (trim_blank_edges(&all_lines).join("\n"), String::new()),
        }
    };

    // The blank run the region OPENS with, carried separately for the same
    // reason `gap` is: `join("\n")` cannot represent it and the trim above
    // discards it. Nothing downstream re-synthesizes this edge, so dropping it
    // deleted a line every version wrote — `package foo;` counts as an import
    // line, so a Java file that opens with a blank has its whole prefix inside
    // that blank run.
    let lead_blanks = |content: &str| -> usize {
        content
            .lines()
            .take_while(|l| l.trim().is_empty())
            .count()
            .min(content.lines().count())
    };
    let lead = widest_gap(lead_blanks(base), lead_blanks(ours), lead_blanks(theirs));

    // And the blank run between the last import and whatever follows it. It was
    // a hard-coded single blank line, which is a formatting opinion the merge
    // is not entitled to: a file that puts two blank lines between its imports
    // and its class javadoc came back with one, and one that puts none came
    // back with one.
    let suffix_gap = |content: &str| -> usize {
        let lines: Vec<&str> = content.lines().collect();
        match lines.iter().rposition(|l| is_import_line(l)) {
            Some(last) => lines[last + 1..]
                .iter()
                .take_while(|l| l.trim().is_empty())
                .count(),
            None => 0,
        }
    };
    let tail_gap = widest_gap(suffix_gap(base), suffix_gap(ours), suffix_gap(theirs));

    let (base_prefix, base_suffix) = extract_prefix_suffix(base);
    let (ours_prefix, ours_suffix) = extract_prefix_suffix(ours);
    let (theirs_prefix, theirs_suffix) = extract_prefix_suffix(theirs);

    // How many blank lines stood between the header and the first import?
    // `package foo;` counts as an import line (`is_import_line`), so for Java
    // this is exactly the blank between the license block and the package
    // declaration — the one the rebuild used to swallow. Take the widest gap
    // any version had: a side that *added* the header (a fresh license block
    // is a common real-world edit) is the only side that has an opinion about
    // the gap below it, and it is not always the skeleton.
    let header_gap = |content: &str| -> usize {
        let lines: Vec<&str> = content.lines().collect();
        match lines.iter().position(|l| is_import_line(l)) {
            Some(first) => lines[..first]
                .iter()
                .rev()
                .take_while(|l| l.trim().is_empty())
                .count(),
            None => 0,
        }
    };
    let gap = header_gap(ours)
        .max(header_gap(theirs))
        .max(header_gap(base));

    // Merge prefix lines (directives before imports)
    let mut result = String::new();
    // The opening blank run first, then the prefix, then the run that abuts the
    // import block. The three are disjoint: when the prefix has no non-blank
    // content at all, `lead` and `gap` name the same run and only `lead` is
    // emitted, because the `gap` below is inside the prefix branch.
    for _ in 0..lead {
        result.push('\n');
    }
    if !base_prefix.is_empty() || !ours_prefix.is_empty() || !theirs_prefix.is_empty() {
        let merged_prefix = match diffy::merge(&base_prefix, &ours_prefix, &theirs_prefix) {
            Ok(m) => m,
            Err(conflicted) => conflicted,
        };
        if !merged_prefix.trim().is_empty() {
            result.push_str(&merged_prefix);
            result.push('\n');
            for _ in 0..gap {
                result.push('\n');
            }
        }
    }

    result.push_str(&import_block);

    // Merge suffix lines (non-import lines after imports)
    if !base_suffix.is_empty() || !ours_suffix.is_empty() || !theirs_suffix.is_empty() {
        let merged_suffix = match diffy::merge(&base_suffix, &ours_suffix, &theirs_suffix) {
            Ok(m) => m,
            Err(conflicted) => conflicted,
        };
        if !merged_suffix.trim().is_empty() {
            result.push('\n');
            for _ in 0..tail_gap {
                result.push('\n');
            }
            result.push_str(&merged_suffix);
        }
    }
    let ours_trailing = ours.len() - ours.trim_end_matches('\n').len();
    let result_trailing = result.len() - result.trim_end_matches('\n').len();
    for _ in result_trailing..ours_trailing {
        result.push('\n');
    }
    (result, order_conflict)
}

/// Merge imports when multi-line import blocks are involved.
/// Matches imports by source module, merges specifiers as a set.
fn merge_imports_with_multiline(
    _base_raw: &str,
    ours_raw: &str,
    _theirs_raw: &str,
    base_imports: &[ImportStatement],
    ours_imports: &[ImportStatement],
    theirs_imports: &[ImportStatement],
) -> String {
    // Build source → specifier sets for base and theirs.
    // Use entry API to merge specifiers when multiple imports share the same source
    // (e.g. `import type { Foo } from "./foo"` AND `import { type a } from "./foo"`).
    let mut base_specs: HashMap<&str, HashSet<&str>> = HashMap::new();
    for imp in base_imports {
        let entry = base_specs.entry(imp.source.as_str()).or_default();
        for s in &imp.specifiers {
            entry.insert(s.as_str());
        }
    }

    // Theirs' specifiers are kept as a SEQUENCE, not a set. A specifier theirs
    // added is appended to ours' list, so the append order is output bytes; a
    // `HashSet` iteration made it a function of the process's random hash seed
    // and the merge stopped being a function of its inputs — output bytes
    // would differ between two runs on identical input. The order theirs
    // wrote them in is the only order either version actually asserts.
    let mut theirs_specs: HashMap<&str, Vec<&str>> = HashMap::new();
    for imp in theirs_imports {
        let entry: &mut Vec<&str> = theirs_specs.entry(imp.source.as_str()).or_default();
        for s in &imp.specifiers {
            if !entry.contains(&s.as_str()) {
                entry.push(s.as_str());
            }
        }
    }

    // Single-line import tracking: base lines and theirs-deleted
    let base_single: HashSet<String> = base_imports
        .iter()
        .filter(|i| !i.is_multiline)
        .map(|i| i.lines[0].clone())
        .collect();
    let theirs_single: HashSet<String> = theirs_imports
        .iter()
        .filter(|i| !i.is_multiline)
        .map(|i| i.lines[0].clone())
        .collect();
    let theirs_deleted_single: HashSet<&str> = base_single
        .iter()
        .filter(|l| !theirs_single.contains(l.as_str()))
        .map(|l| l.as_str())
        .collect();

    // Process ours imports, merging in theirs specifiers
    let mut result_parts: Vec<String> = Vec::new();
    let mut handled_theirs_sources: HashSet<&str> = HashSet::new();

    // Walk through ours_raw to preserve formatting (blank lines, comments)
    let lines: Vec<&str> = ours_raw.lines().collect();
    let mut i = 0;
    let mut ours_imp_idx = 0;

    while i < lines.len() {
        let line = lines[i];

        if line.trim().is_empty() {
            result_parts.push(line.to_string());
            i += 1;
            continue;
        }

        if is_import_line(line) {
            let trimmed = line.trim();
            let starts_multiline = (trimmed.contains('{') && !trimmed.contains('}'))
                || (trimmed.starts_with("import (") && !trimmed.contains(')'))
                || (trimmed.starts_with("from ")
                    && trimmed.contains("import (")
                    && !trimmed.contains(')'));

            if starts_multiline && ours_imp_idx < ours_imports.len() {
                let imp = &ours_imports[ours_imp_idx];
                // Find the matching import by source
                let source = imp.source.as_str();
                handled_theirs_sources.insert(source);

                // Merge specifiers: ours + theirs additions - theirs deletions
                let base_spec_set = base_specs.get(source).cloned().unwrap_or_default();
                let theirs_seq: &[&str] =
                    theirs_specs.get(source).map(Vec::as_slice).unwrap_or(&[]);
                // Added by theirs: in theirs but not in base, in theirs' order.
                let theirs_added: Vec<&str> = theirs_seq
                    .iter()
                    .copied()
                    .filter(|s| !base_spec_set.contains(s))
                    .collect();
                // Deleted by theirs: in base but not in theirs
                let theirs_removed: HashSet<&str> = base_spec_set
                    .iter()
                    .copied()
                    .filter(|s| !theirs_seq.contains(s))
                    .collect();

                // Final set: ours (in original order) + theirs_added - theirs_removed
                let mut final_specs: Vec<&str> = imp
                    .specifiers
                    .iter()
                    .map(|s| s.as_str())
                    .filter(|s| !theirs_removed.contains(s))
                    .collect();
                for added in &theirs_added {
                    if !final_specs.contains(added) {
                        final_specs.push(added);
                    }
                }

                // Detect indentation from the original block
                let indent = if imp.lines.len() > 1 {
                    let second = &imp.lines[1];
                    &second[..second.len() - second.trim_start().len()]
                } else {
                    "     "
                };

                // Reconstruct multi-line import
                result_parts.push(imp.lines[0].clone()); // `import {`
                for spec in &final_specs {
                    result_parts.push(format!("{}{},", indent, spec));
                }
                // Closing line from ours
                if let Some(last) = imp.lines.last() {
                    result_parts.push(last.clone());
                }

                // Skip past the original multi-line block in ours_raw
                let close_char = if trimmed.contains('{') { '}' } else { ')' };
                i += 1;
                while i < lines.len() {
                    if lines[i].trim().starts_with(close_char) {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                ours_imp_idx += 1;
                continue;
            } else {
                // Single-line import
                if ours_imp_idx < ours_imports.len() {
                    let imp = &ours_imports[ours_imp_idx];
                    let source = imp.source.as_str();
                    handled_theirs_sources.insert(source);
                    ours_imp_idx += 1;

                    // Check if theirs has a multi-line version with more specifiers
                    if let Some(theirs_seq) = theirs_specs.get(source) {
                        let base_spec_set = base_specs.get(source).cloned().unwrap_or_default();
                        // Theirs' order, again: see the multi-line arm above.
                        let theirs_added: Vec<&str> = theirs_seq
                            .iter()
                            .copied()
                            .filter(|s| !base_spec_set.contains(s))
                            .collect();

                        if !theirs_added.is_empty() {
                            // Parse ours single-line specifier from the line text.
                            //
                            // The specifiers and the ` import ` position they were
                            // parsed from travel as ONE value. The reconstruction
                            // below needs that same position to rebuild the prefix,
                            // and it used to re-`find` it and `.unwrap()` the
                            // result — a slice whose precondition ("this line
                            // contains ` import `") is a property of the input file,
                            // stated nowhere, on the merge path, and a byte slice
                            // computed from a re-`find` can land off a char boundary
                            // and panic. Binding the two together makes "we found
                            // specifiers" and "we know where they start" the same
                            // fact, so there is no second lookup to disagree.
                            let trimmed_line = line.trim();
                            // Python: "from X import Y, Z" → extract after "import "
                            let ours_parse: Option<(usize, Vec<&str>)> = trimmed_line
                                .find(" import ")
                                .map(|import_pos| {
                                    let specs: Vec<&str> = trimmed_line[import_pos + 8..]
                                        .split(',')
                                        .map(|spec| spec.trim().trim_end_matches(';'))
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                    (import_pos, specs)
                                })
                                // JS/TS: "import X from 'Y'" → X is a default import,
                                // not a specifier. Only merge if we found named
                                // specifiers.
                                .filter(|(_, specs)| !specs.is_empty());
                            if let Some((import_pos, ours_specifiers)) = ours_parse {
                                let mut final_specs: Vec<&str> = ours_specifiers.clone();
                                for added in &theirs_added {
                                    if !final_specs.contains(added) {
                                        final_specs.push(added);
                                    }
                                }
                                // Find theirs import to get formatting
                                if let Some(theirs_imp) =
                                    theirs_imports.iter().find(|ti| ti.source == source)
                                {
                                    if theirs_imp.is_multiline {
                                        // Use theirs's multi-line formatting
                                        let indent = if theirs_imp.lines.len() > 1 {
                                            let second = &theirs_imp.lines[1];
                                            &second[..second.len() - second.trim_start().len()]
                                        } else {
                                            "    "
                                        };
                                        // Reconstruct as multi-line using theirs's opening/closing style
                                        result_parts.push(theirs_imp.lines[0].clone());
                                        for spec in &final_specs {
                                            result_parts.push(format!("{}{},", indent, spec));
                                        }
                                        if let Some(last) = theirs_imp.lines.last() {
                                            result_parts.push(last.clone());
                                        }
                                        i += 1;
                                        continue;
                                    }
                                }
                                // Theirs not multi-line but has more specifiers: reconstruct single-line
                                // e.g. "from X import A, B, C" — the prefix is the
                                // match `ours_parse` already made, not a fresh one.
                                let prefix = &trimmed_line[..import_pos + 8];
                                result_parts.push(format!("{}{}", prefix, final_specs.join(", ")));
                                i += 1;
                                continue;
                            }
                        }
                    }
                } else {
                    // No more ours imports to match
                }
                // Check if theirs deleted this single-line import
                if !theirs_deleted_single.contains(line) {
                    result_parts.push(line.to_string());
                }
            }
        } else {
            result_parts.push(line.to_string());
        }
        i += 1;
    }

    // Add any new imports from theirs that have new sources
    for imp in theirs_imports {
        if handled_theirs_sources.contains(imp.source.as_str()) {
            continue;
        }
        // Truly new import from theirs (source wasn't handled in the main loop)
        for line in &imp.lines {
            result_parts.push(line.clone());
        }
    }

    let mut result = result_parts.join("\n");

    // Non-import lines: use diffy 3-way merge so adds/deletes/edits on
    // either side are handled correctly (fixes #60).
    let extract_non_imports = |content: &str| -> String {
        content
            .lines()
            .filter(|l| !l.trim().is_empty() && !is_import_line(l))
            .filter(|l| {
                let t = l.trim();
                // Exclude multi-line import continuation lines:
                // - specifier lines ending with comma (but not assignments)
                // - bare closing parens/braces
                // - closing lines like `} from "./foo"` or `) from "bar"`
                if (t.ends_with(',') && !t.contains('=')) || t == ")" || t == "}" {
                    return false;
                }
                // Closing line of JS/TS multi-line import: `} from "..."` or `} from '...'`
                if t.starts_with('}') && t.contains("from ") {
                    return false;
                }
                // Closing line of Python multi-line import: `) ` at end or just `)`
                if t.starts_with(')') {
                    return false;
                }
                true
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let base_ni = extract_non_imports(_base_raw);
    let ours_ni = extract_non_imports(ours_raw);
    let theirs_ni = extract_non_imports(_theirs_raw);

    if !base_ni.is_empty() || !ours_ni.is_empty() || !theirs_ni.is_empty() {
        let merged_ni = match diffy::merge(&base_ni, &ours_ni, &theirs_ni) {
            Ok(m) => m,
            Err(conflicted) => conflicted,
        };
        if !merged_ni.trim().is_empty() {
            result.push('\n');
            result.push('\n');
            result.push_str(&merged_ni);
        }
    }

    let ours_trailing = ours_raw.len() - ours_raw.trim_end_matches('\n').len();
    let result_trailing = result.len() - result.trim_end_matches('\n').len();
    for _ in result_trailing..ours_trailing {
        result.push('\n');
    }
    result
}

/// Extract the source/module prefix from an import line for group matching.
/// e.g. "from collections import OrderedDict" -> "collections"
///      "import React from 'react'" -> "react"
///      "use std::collections::HashMap;" -> "std::collections"
fn import_source_prefix(line: &str) -> &str {
    // For multi-line imports, search all lines for the source module
    // (e.g. `} from "./foo"` on the closing line)
    for l in line.lines() {
        let trimmed = l.trim();
        // Python: "from X import Y" -> X
        if let Some(rest) = trimmed.strip_prefix("from ") {
            return rest.split_whitespace().next().unwrap_or("");
        }
        // JS/TS closing line: `} from 'Y'` or `} from "Y"`
        if trimmed.starts_with('}') && trimmed.contains("from ") {
            if let Some(quote_start) = trimmed.find(['\'', '"']) {
                let after = &trimmed[quote_start + 1..];
                if let Some(quote_end) = after.find(['\'', '"']) {
                    return &after[..quote_end];
                }
            }
        }
        // JS/TS: "import X from 'Y'" -> Y (between quotes)
        if let Some(rest) = trimmed.strip_prefix("import ") {
            if let Some(quote_start) = trimmed.find(['\'', '"']) {
                let after = &trimmed[quote_start + 1..];
                if let Some(quote_end) = after.find(['\'', '"']) {
                    return &after[..quote_end];
                }
            }
            // Java/Kotlin: "import a.b.C;" or "import static a.b.C.d;" -> the
            // dotted path. There are no quotes to key on, and returning the
            // whole line (the old fallback) made every such import look
            // unrelated to every other one.
            let rest = rest.strip_prefix("static ").unwrap_or(rest);
            let path = rest
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches(';');
            if !path.is_empty() && path.contains('.') {
                return path;
            }
        }
        // Rust: "use X::Y;" -> X
        if let Some(rest) = trimmed.strip_prefix("use ") {
            return rest.split("::").next().unwrap_or("").trim_end_matches(';');
        }
    }
    line.trim()
}

/// How many leading source-path segments two import lines share.
///
/// The unit of relatedness for imports is the path prefix, not the whole
/// source: `java.util.Map` belongs with `java.util.List` (2 segments shared),
/// not with `com.google.common.collect.Maps` (0). Segments are split on the
/// separators every ecosystem uses for the same purpose — `.` for Java and
/// Python, `/` for JS/TS module specifiers, `::` for Rust — so one function
/// serves all of them.
fn import_prefix_affinity(a: &str, b: &str) -> usize {
    fn segments(line: &str) -> Vec<&str> {
        import_source_prefix(line)
            .trim_matches(|c| c == '.' || c == '/')
            .split(['.', '/', ':'])
            .filter(|s| !s.is_empty())
            .collect()
    }
    segments(a)
        .iter()
        .zip(segments(b).iter())
        .take_while(|(x, y)| x == y)
        .count()
}

/// Fallback to line-level 3-way merge when entity extraction isn't possible.
///
/// Inserts newlines around syntactic separators ({, }, ;) so that changes in
/// different code blocks align independently before line-level merge, reducing
/// spurious conflicts.
///
/// Separator expansion is skipped for data formats (JSON, YAML, TOML, lock
/// files) where `{`, `}`, `;` are structural content rather than code
/// separators. Expanding them destroys alignment and produces far more
/// conflicts.
pub(crate) fn line_level_fallback(
    base: &str,
    ours: &str,
    theirs: &str,
    file_path: &str,
    host: &Host,
) -> MergeResult {
    let mut stats = MergeStats::default();
    stats.mark_fallback();

    // Skip separator expansion for data formats where {/}/; are content, not
    // separators — and for any input that already carries the marker byte, the
    // one case where the expansion would not be invertible.
    let skip = skip_expansion(file_path) || !expansion_safe(base, ours, theirs);

    if skip {
        // Use git merge-file for data formats so we match git's output exactly.
        // diffy::merge uses a different diff algorithm that can produce more
        // conflict markers on structured data like lock files.
        return line_merge_file(base, ours, theirs, stats, host);
    }

    // Try separator expansion + diffy first, then compare against git merge-file.
    // Use whichever produces fewer conflict markers so we're never worse than git.
    let base_expanded = expand_separators(base);
    let ours_expanded = expand_separators(ours);
    let theirs_expanded = expand_separators(theirs);

    let expanded_result = match diffy::merge(&base_expanded, &ours_expanded, &theirs_expanded) {
        Ok(merged) => {
            let content = collapse_separators(&merged);
            Some(MergeResult {
                content,
                conflicts: vec![],
                warnings: vec![],
                stats: stats.clone(),
                audit: vec![],
            })
        }
        Err(_) => {
            // Separator expansion conflicted, try plain diffy
            match diffy::merge(base, ours, theirs) {
                Ok(merged) => Some(MergeResult {
                    content: merged,
                    conflicts: vec![],
                    warnings: vec![],
                    stats: stats.clone(),
                    audit: vec![],
                }),
                Err(conflicted) => {
                    let mut s = stats.clone();
                    s.entities_conflicted = 1;
                    Some(MergeResult {
                        content: conflicted,
                        conflicts: vec![EntityConflict {
                            entity_name: "(file)".to_string(),
                            entity_type: "file".to_string(),
                            kind: ConflictKind::BothModified,
                            complexity: classify_conflict(Some(base), Some(ours), Some(theirs)),
                            ours_content: Some(ours.to_string()),
                            theirs_content: Some(theirs.to_string()),
                            base_content: Some(base.to_string()),
                        }],
                        warnings: vec![],
                        stats: s,
                        audit: vec![],
                    })
                }
            }
        }
    };

    // Get the line-level merge as our floor
    let git_result = line_merge_file(base, ours, theirs, stats, host);

    // Compare: use expanded result only if it has fewer or equal markers
    match expanded_result {
        Some(expanded) if expanded.conflicts.is_empty() && !git_result.conflicts.is_empty() => {
            // Separator expansion resolved cleanly, git did not: use it
            expanded
        }
        Some(expanded) if !expanded.conflicts.is_empty() && !git_result.conflicts.is_empty() => {
            // Both conflicted: use whichever has fewer markers
            let expanded_markers = expanded
                .content
                .lines()
                .filter(|l| l.starts_with("<<<<<<<"))
                .count();
            let git_markers = git_result
                .content
                .lines()
                .filter(|l| l.starts_with("<<<<<<<"))
                .count();
            if expanded_markers <= git_markers {
                expanded
            } else {
                git_result
            }
        }
        _ => git_result,
    }
}

/// The line-level merge, taken through whatever route the caller granted.
///
/// Used instead of the in-process line merge for data formats (lock files,
/// JSON, YAML, TOML) where weave cannot improve on git: `diffy` uses a
/// different diff algorithm that can produce more conflict markers on
/// structured data (e.g. 22 markers vs git's 19 on uv.lock). The conflicted
/// output is the part that cannot go in-process at all — see
/// [`crate::host::git_line_merge`].
///
/// With no route granted, this is [`diffy_fallback`]: the same answer the
/// caller already got whenever the scratch files could not be written.
///
/// The summary comes in by value and leaves inside the result. It used to come
/// in as `&mut`, and the authority that granted — to write a counter the caller
/// could still read afterwards — was exercised by nobody: all three call sites
/// either return this result directly or drop their `stats` immediately after,
/// so the only reader of the write was the `MergeResult` this function builds.
/// Owning it says that, drops two clones, and leaves no handle behind for a
/// later stage to disagree through.
pub(crate) fn line_merge_file(
    base: &str,
    ours: &str,
    theirs: &str,
    mut stats: MergeStats,
    host: &Host,
) -> MergeResult {
    let Some(line_merge) = host.line_merge else {
        return diffy_fallback(base, ours, theirs, stats);
    };
    let Some(merged) = line_merge(LineMergeStyle::Labelled, base, ours, theirs) else {
        return diffy_fallback(base, ours, theirs, stats);
    };

    if merged.clean {
        MergeResult {
            content: merged.content,
            conflicts: vec![],
            warnings: vec![],
            stats,
            audit: vec![],
        }
    } else {
        stats.entities_conflicted = 1;
        MergeResult {
            content: merged.content,
            conflicts: vec![EntityConflict {
                entity_name: "(file)".to_string(),
                entity_type: "file".to_string(),
                kind: ConflictKind::BothModified,
                complexity: classify_conflict(Some(base), Some(ours), Some(theirs)),
                ours_content: Some(ours.to_string()),
                theirs_content: Some(theirs.to_string()),
                base_content: Some(base.to_string()),
            }],
            warnings: vec![],
            stats,
            audit: vec![],
        }
    }
}

/// The in-process line merge, used when no line-level route was granted or
/// the granted one could not answer.
fn diffy_fallback(base: &str, ours: &str, theirs: &str, mut stats: MergeStats) -> MergeResult {
    match diffy::merge(base, ours, theirs) {
        Ok(merged) => MergeResult {
            content: merged,
            conflicts: vec![],
            warnings: vec![],
            stats,
            audit: vec![],
        },
        Err(conflicted) => {
            stats.entities_conflicted = 1;
            MergeResult {
                content: conflicted,
                conflicts: vec![EntityConflict {
                    entity_name: "(file)".to_string(),
                    entity_type: "file".to_string(),
                    kind: ConflictKind::BothModified,
                    complexity: classify_conflict(Some(base), Some(ours), Some(theirs)),
                    ours_content: Some(ours.to_string()),
                    theirs_content: Some(theirs.to_string()),
                    base_content: Some(base.to_string()),
                }],
                warnings: vec![],
                stats,
                audit: vec![],
            }
        }
    }
}

/// Whether one name repeats often enough that identity has stopped meaning
/// anything in this file.
///
/// Not a liveness guard: the v2 matcher terminates regardless. The reason is
/// honesty — with a name repeated past the threshold, a per-name matcher can
/// still produce a verdict for every cell, and every one of those verdicts is
/// fiction. So the file takes the typed `Unsupported::AmbiguousIdentity` route
/// (`v2::mod`) and a line-level merge, rather than a confident wrong answer.
/// The threshold arrives on the [`Host`]; it used to be read out of
/// `WEAVE_MAX_DUPLICATES` right here, in the middle of the decision.
pub(crate) fn has_excessive_duplicates(entities: &[SemanticEntity], threshold: usize) -> bool {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for e in entities {
        *counts.entry(&e.name).or_default() += 1;
    }
    counts.values().any(|&c| c >= threshold)
}

/// Filter out entities that are nested inside other entities.
///
/// When a class contains methods which contain local variables, sem-core may
/// extract all of them as entities. But for merge purposes a nested entity is
/// part of its parent — they are handled by inner entity merge. Keeping them
/// causes false conflicts (two methods both declaring `const user` would
/// appear as BothAdded). O(n log n) via sort + stack.
pub(crate) fn filter_nested_entities(mut entities: Vec<SemanticEntity>) -> Vec<SemanticEntity> {
    if entities.len() <= 1 {
        return entities;
    }

    // Sort by start_line ASC, then by end_line DESC (widest span first).
    // A parent entity always appears before its children in this order.
    entities.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(b.end_line.cmp(&a.end_line))
    });

    // Stack-based filter: track the end_line of the current outermost entity.
    let mut result: Vec<SemanticEntity> = Vec::with_capacity(entities.len());
    let mut max_end: usize = 0;

    for entity in entities {
        if entity.start_line > max_end || max_end == 0 {
            // Not nested: new top-level entity
            max_end = entity.end_line;
            result.push(entity);
        } else if entity.start_line == result.last().map_or(0, |e| e.start_line)
            && entity.end_line == result.last().map_or(0, |e| e.end_line)
        {
            // Exact same span (e.g. decorated_definition wrapping function_definition)
            result.push(entity);
        }
        // else: strictly nested, skip
    }

    result
}

/// Get child entities of a parent, sorted by start line.
pub(crate) fn get_child_entities<'a>(
    parent: &SemanticEntity,
    all_entities: &'a [SemanticEntity],
) -> Vec<&'a SemanticEntity> {
    let mut children: Vec<&SemanticEntity> = all_entities
        .iter()
        .filter(|e| e.parent_id.as_deref() == Some(&parent.id))
        .collect();
    children.sort_by_key(|e| e.start_line);
    children
}

/// Check if an entity type is a container that may benefit from inner entity merge.
pub(crate) fn is_container_entity_type(entity_type: &str) -> bool {
    matches!(
        entity_type,
        "class"
            | "interface"
            | "enum"
            | "impl"
            | "trait"
            | "module"
            | "impl_item"
            | "trait_item"
            | "struct"
            | "union"
            | "namespace"
            | "struct_item"
            | "struct_specifier"
            | "variable"
            | "export"
    )
}

/// What a scoped marker knows about where it sits: the format it is written in,
/// and the entity whose body the scope is inside.
///
/// A statement-scoped conflict is a strictly better artefact than a
/// whole-entity one — three lines in markers instead of a whole method — but it
/// is only better to READ if it says the same things. The boundary contract
/// requires a `refused_by:` line after every `<<<<<<< ours`, in the file's own
/// comment syntax, and a reader who is handed three anonymous lines has lost
/// the one thing the whole-entity marker gave them: which declaration this
/// is.
#[derive(Clone, Copy)]
pub(crate) struct ScopeMarkers<'a> {
    pub fmt: &'a MarkerFormat,
    /// The entity whose body this scope is inside — `""` when the caller has no
    /// name for it, in which case the marker simply omits the context.
    pub entity_type: &'a str,
    pub entity_name: &'a str,
    /// Which guard refused this scope. A scoped marker is emitted by exactly
    /// two callers — the statement fold and the container merge — and they
    /// refuse for different reasons, so the reason travels with the format
    /// rather than being re-derived from the shape of the text.
    pub guard: &'static str,
}

impl<'a> ScopeMarkers<'a> {
    /// A scope with no entity context: the marker names the scope and says
    /// nothing about where it sits. Used by the fold's own tests, and by any
    /// caller merging a body it has no name for.
    #[cfg(test)]
    pub(crate) fn bare(fmt: &'a MarkerFormat) -> Self {
        Self {
            fmt,
            entity_type: "",
            entity_name: "",
            guard: "statement_fold",
        }
    }

    /// The same format, now inside a named entity.
    pub(crate) fn inside(
        fmt: &'a MarkerFormat,
        entity_type: &'a str,
        entity_name: &'a str,
        guard: &'static str,
    ) -> Self {
        Self {
            fmt,
            entity_type,
            entity_name,
            guard,
        }
    }

    /// ` in function `f`` — where this scope sits, empty when the caller has no
    /// name for it.
    ///
    /// This used to be the tail of the marker's `hint:` line. The hint line is
    /// gone, stripped as litter, but *which declaration a scope sits in* is
    /// not advice — it is the one thing a reader of `scope `then`` cannot
    /// recover from the text around it. So it moved up onto the opening
    /// marker, where the whole-entity marker already states the same fact,
    /// instead of leaving with the sentence it happened to be attached to.
    fn context(&self) -> String {
        if self.entity_name.is_empty() {
            String::new()
        } else if self.entity_type.is_empty() {
            format!(" in `{}`", self.entity_name)
        } else {
            format!(" in {} `{}`", self.entity_type, self.entity_name)
        }
    }
}

/// Generate a scoped conflict marker for a single member within a container merge.
pub(crate) fn scoped_conflict_marker(
    name: &str,
    base: Option<&str>,
    ours: Option<&str>,
    theirs: Option<&str>,
    ours_deleted: bool,
    theirs_deleted: bool,
    scope: &ScopeMarkers<'_>,
) -> String {
    let fmt = scope.fmt;
    let open = "<".repeat(fmt.marker_length);
    let sep = "=".repeat(fmt.marker_length);
    let close = ">".repeat(fmt.marker_length);

    // ONE cut, the same one `EntityConflict::to_conflict_markers` takes. This
    // function used to carry its own copy of the narrowing, and a copy of a rule
    // is a second rule as soon as either copy is edited.
    let hole = crate::conflict::ConflictBox::cut(
        ours,
        (!fmt.enhanced).then_some(base.unwrap_or("")),
        theirs,
    );

    let mut out = String::new();
    crate::conflict::push_lines(&mut out, hole.frame_prefix());

    // Opening marker. In enhanced mode it carries the same three things the
    // whole-entity marker carries — what this is, how hard it looks, and a
    // `refused_by:` line in the file's own comment syntax — because the boundary
    // contract is entitled to them on EVERY conflict, not only on the ones
    // that happen to be entity-sized.
    if fmt.enhanced {
        let complexity = crate::conflict::classify_conflict(base, ours, theirs);
        let state = if ours_deleted {
            ", deleted in ours"
        } else if theirs_deleted {
            ", deleted in theirs"
        } else {
            ""
        };
        out.push_str(&format!(
            "{} ours \u{2014} scope `{}`{}{} ({}, confidence: {})\n",
            open,
            name,
            scope.context(),
            state,
            complexity,
            complexity.confidence()
        ));
        out.push_str(&crate::conflict::refusal_line(
            &fmt.comment_prefix,
            scope.guard,
            base,
            ours,
            theirs,
        ));
    } else {
        out.push_str(&format!("{} ours\n", open));
    }

    crate::conflict::push_lines(
        &mut out,
        hole.side(crate::conflict::BoxSide::Ours).unwrap_or(&[]),
    );

    // Base section for diff3 format (standard mode only). It is cut at the same
    // offsets as the other two sides — it was one of the sides the cut was taken
    // over — so `prefix ++ base_hole ++ suffix` is base, byte for byte. The old
    // code sliced base at *ours'* offsets and said so: "use prefix/suffix from
    // ours/theirs narrowing as approximation".
    if !fmt.enhanced {
        out.push_str(&format!("{} base\n", "|".repeat(fmt.marker_length)));
        crate::conflict::push_lines(
            &mut out,
            hole.side(crate::conflict::BoxSide::Base).unwrap_or(&[]),
        );
    }

    out.push_str(&format!("{}\n", sep));
    crate::conflict::push_lines(
        &mut out,
        hole.side(crate::conflict::BoxSide::Theirs).unwrap_or(&[]),
    );

    if fmt.enhanced {
        let state = if theirs_deleted {
            " deleted in theirs"
        } else if ours_deleted {
            " deleted in ours"
        } else {
            ""
        };
        out.push_str(&format!(
            "{} theirs \u{2014} scope `{}`{}\n",
            close, name, state
        ));
    } else {
        out.push_str(&format!("{} theirs\n", close));
    }

    crate::conflict::push_lines(&mut out, hole.frame_suffix());
    out
}

/// Detect whether a container entity uses Python-style indentation (`:` terminated
/// declaration) or brace-delimited style (`{`). Only inspects the declaration
/// line(s), not the body, so dict literals / set comprehensions inside methods
/// don't cause a false negative.
fn is_python_style_container(lines: &[&str]) -> bool {
    // Find the first line that looks like a class/def/async def declaration
    let has_colon_decl = lines.iter().any(|l| {
        let trimmed = l.trim();
        (trimmed.starts_with("class ")
            || trimmed.starts_with("def ")
            || trimmed.starts_with("async def "))
            && trimmed.ends_with(':')
    });
    if !has_colon_decl {
        return false;
    }
    // Verify the container itself opens with `:`, not `{`.
    // Look at the declaration line (first line ending with `:` that is class/def).
    // If that line also contains `{` it's not Python style (edge case: shouldn't happen).
    if let Some(decl) = lines.iter().find(|l| {
        let t = l.trim();
        (t.starts_with("class ") || t.starts_with("def ") || t.starts_with("async def "))
            && t.ends_with(':')
    }) {
        !decl.contains('{')
    } else {
        false
    }
}

/// Returns true if a (trimmed) line closes a braced container body.
///
/// Matches a leading `}` followed only by closing punctuation, so it accepts
/// the bare `}` / `};` / `},` forms as well as call- and index-wrapped closers
/// like `})`, `});`, `}),`, `])`. This lets object literals passed as call
/// arguments (`configure({ ... })`, `defineConfig({ ... })`) decompose into
/// per-member chunks instead of collapsing into one whole-entity conflict.
pub(crate) fn is_container_close_line(trimmed: &str) -> bool {
    let mut chars = trimmed.chars();
    if chars.next() != Some('}') {
        return false;
    }
    chars.all(|c| matches!(c, ')' | ']' | ';' | ',' | ' ' | '\t'))
}

/// Extract the header (class declaration) and footer (closing brace) from a
/// container, so the members between them can be merged on their own.
///
/// Handles both brace-delimited (JS/TS/Java/Rust/C) and indentation-based
/// (Python) containers — which one this is comes from
/// [`is_python_style_container`], read off the declaration line only.
pub(crate) fn extract_container_wrapper(content: &str) -> Option<(&str, &str)> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 2 {
        return None;
    }

    // Check if this is a Python-style container (ends with `:` instead of `{`)
    let is_python_style = is_python_style_container(&lines);

    if is_python_style {
        // Python: header is the `class Foo:` line, no footer
        let header_end = lines.iter().position(|l| l.trim().ends_with(':'))?;
        let header_byte_end: usize = lines[..=header_end].iter().map(|l| l.len() + 1).sum();
        let header = &content[..header_byte_end.min(content.len())];
        // No closing brace in Python — footer is empty
        let footer = &content[content.len()..];
        Some((header, footer))
    } else {
        // Brace-delimited: header up to `{`, footer from last `}`
        let header_end = lines.iter().position(|l| l.contains('{'))?;
        let header_byte_end = lines[..=header_end]
            .iter()
            .map(|l| l.len() + 1)
            .sum::<usize>();
        let header = &content[..header_byte_end.min(content.len())];

        let footer_start = lines
            .iter()
            .rposition(|l| is_container_close_line(l.trim()))?;

        let footer_byte_start: usize = lines[..footer_start].iter().map(|l| l.len() + 1).sum();
        let footer = &content[footer_byte_start.min(content.len())..];

        Some((header, footer))
    }
}

/// Extract a member name from a declaration line.
pub(crate) fn extract_member_name(line: &str) -> String {
    let trimmed = line.trim();

    // Go method receiver: `func (c *Calculator) Add(` -> skip receiver, find name before second `(`
    if trimmed.starts_with("func ") && trimmed.get(5..6) == Some("(") {
        // Skip past the receiver: find closing `)`, then extract name before next `(`
        if let Some(recv_close) = trimmed.find(')') {
            let after_recv = &trimmed[recv_close + 1..];
            if let Some(paren_pos) = after_recv.find('(') {
                let before = after_recv[..paren_pos].trim();
                let name: String = before
                    .chars()
                    .rev()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                if !name.is_empty() {
                    return name;
                }
            }
        }
    }

    // Strategy 1: For method/function declarations with parentheses,
    // the name is the identifier immediately before `(`.
    // This handles all languages: Java `public int add(`, Rust `pub fn add(`,
    // Python `def add(`, TS `async getUser(`, Go `func add(`, etc.
    if let Some(paren_pos) = trimmed.find('(') {
        let before = trimmed[..paren_pos].trim_end();
        let name: String = before
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if !name.is_empty() {
            return name;
        }
    }

    // Strategy 2: For fields/properties/variants without parens,
    // strip keywords and take the first identifier.
    let mut s = trimmed;
    for keyword in &[
        "export ",
        "public ",
        "private ",
        "protected ",
        "static ",
        "abstract ",
        "async ",
        "override ",
        "readonly ",
        "pub ",
        "pub(crate) ",
        "fn ",
        "def ",
        "get ",
        "set ",
    ] {
        if s.starts_with(keyword) {
            s = &s[keyword.len()..];
        }
    }
    if s.starts_with("fn ") {
        s = &s[3..];
    }

    let name: String = s
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();

    if name.is_empty() {
        trimmed.chars().take(20).collect()
    } else {
        name
    }
}

/// For anonymous struct literal entries (e.g., Go slice entries starting with `{`),
/// derive a name from the first key-value field inside the chunk.
/// E.g., `{ Name: "panelTitleSearch", ... }` → `panelTitleSearch`
pub(crate) fn derive_name_from_struct_literal(content: &str) -> Option<String> {
    for line in content.lines().skip(1) {
        let trimmed = line.trim().trim_end_matches(',');
        // Look for `Key: "value"` or `Key: value` pattern
        if let Some(colon_pos) = trimmed.find(':') {
            let value = trimmed[colon_pos + 1..].trim();
            // Strip quotes from string values
            let value = value.trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Is this content binary? A NUL byte in the first 8KB, the same heuristic git
/// uses.
///
/// Public because the merge driver has to answer it too, before it has a
/// result to report — and it was answering it with its own byte-identical copy,
/// so the two crates could have drifted about what weave refuses to touch.
pub fn is_binary(content: &str) -> bool {
    content.as_bytes().iter().take(8192).any(|&b| b == 0)
}

/// Check if content already contains git conflict markers.
/// This happens with AU/AA conflicts where git stores markers in stage blobs.
pub(crate) fn has_conflict_markers(content: &str) -> bool {
    content.contains("<<<<<<<") && content.contains(">>>>>>>")
}

/// Returns true for data/config file formats where separator expansion
/// (`{`, `}`, `;`) is counterproductive because those chars are structural
/// content rather than code block separators.
///
/// Reached only on the line-level fallback route — the one a file takes after
/// the typed `Unsupported` verdict — so it is asked only of files weave has
/// already declined to merge at entity granularity.
///
/// Note: template files like .svelte/.vue are NOT included here because their
/// embedded `<script>` sections contain real code where separator expansion helps.
fn skip_expansion(file_path: &str) -> bool {
    let path_lower = file_path.to_lowercase();
    let extensions = [
        // Data/config formats
        ".json",
        ".yaml",
        ".yml",
        ".toml",
        ".lock",
        ".xml",
        ".csv",
        ".tsv",
        ".ini",
        ".cfg",
        ".conf",
        ".properties",
        ".env",
        // Markup/document formats
        ".md",
        ".markdown",
        ".txt",
        ".rst",
        ".svg",
        ".html",
        ".htm",
    ];
    extensions.iter().any(|ext| path_lower.ends_with(ext))
}

/// The byte that marks a line break the expansion invented.
///
/// U+0001 (SOH) is a C0 control character: no mainstream programming-language
/// grammar admits it outside a string literal, and `expansion_safe` refuses the
/// whole transform on any input that contains one, so a marker in the expanded
/// text can only be one this function wrote.
const EXPANSION_MARK: u8 = 0x01;

/// Can this triple be expanded and collapsed without ambiguity?
///
/// Exactly one precondition: none of the three versions already contains the
/// marker byte. Then `collapse_separators ∘ expand_separators = id`, and every
/// marker the merge sees is the expander's own.
fn expansion_safe(base: &str, ours: &str, theirs: &str) -> bool {
    let has_mark = |s: &str| s.as_bytes().contains(&EXPANSION_MARK);
    !has_mark(base) && !has_mark(ours) && !has_mark(theirs)
}

/// Expand syntactic separators into separate lines for finer merge alignment.
/// Isolating separators lets line-based merge see block boundaries as
/// independent change units.
/// Uses byte-level iteration since separators ({, }, ;) and string delimiters
/// (", ', `) are all ASCII.
///
/// **Every line break this inserts is marked.** The expansion is a lens, not a
/// reformat: the merge happens in the expanded world and the answer is read
/// back in the original one, so the transform has to be exactly invertible. It
/// was not. `collapse_separators` used to guess which separator-only lines it
/// had created, and its one join branch was unreachable (`result` always ends
/// with `\n` at the top of the loop), so collapse was a no-op and any file that
/// took this path came back with every `{`, `}` and `;` on a line of its own
/// and a blank line after each one — text every version agreed on, destroyed by
/// a merge that reported success, and a real regression in files where a
/// separator sits at a line boundary the merge collapses. Marking the
/// inserted breaks makes the inverse a deletion of marked bytes rather than a
/// guess.
fn expand_separators(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut result = Vec::with_capacity(content.len() * 3);
    let mut in_string = false;
    let mut escape_next = false;
    let mut string_char = b'"';

    for &b in bytes {
        if escape_next {
            result.push(b);
            escape_next = false;
            continue;
        }
        if b == b'\\' && in_string {
            result.push(b);
            escape_next = true;
            continue;
        }
        if !in_string && (b == b'"' || b == b'\'' || b == b'`') {
            in_string = true;
            string_char = b;
            result.push(b);
            continue;
        }
        if in_string && b == string_char {
            in_string = false;
            result.push(b);
            continue;
        }

        if !in_string && (b == b'{' || b == b'}' || b == b';') {
            if result.last() != Some(&b'\n') && !result.is_empty() {
                result.push(EXPANSION_MARK);
                result.push(b'\n');
            }
            result.push(b);
            result.push(EXPANSION_MARK);
            result.push(b'\n');
        } else {
            result.push(b);
        }
    }

    // Safe: we only inserted ASCII bytes into valid UTF-8 content
    unsafe { String::from_utf8_unchecked(result) }
}

/// Collapse separator expansion back to original formatting: the exact inverse
/// of [`expand_separators`] under [`expansion_safe`].
///
/// A marked line break is one the expander invented, so undoing the expansion
/// is deleting every `MARK NL` pair — and nothing else. Text the merge carried
/// through from any version keeps its own bytes, including its blank lines and
/// its trailing newline, because this function never writes a byte of its own.
/// A bare marker with no newline after it can only come from a merge that split
/// the pair; dropping it keeps the output free of control characters.
fn collapse_separators(merged: &str) -> String {
    let bytes = merged.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == EXPANSION_MARK {
            // MARK NL is one inserted break; a lone MARK is debris.
            if bytes.get(i + 1) == Some(&b'\n') {
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        result.push(bytes[i]);
        i += 1;
    }
    // Safe: deleting whole ASCII bytes from valid UTF-8 leaves valid UTF-8.
    unsafe { String::from_utf8_unchecked(result) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding::replace_at_word_boundaries;

    #[test]
    fn test_replace_at_word_boundaries() {
        // Should replace standalone occurrences
        assert_eq!(
            replace_at_word_boundaries("fn get() {}", "get", "__E__"),
            "fn __E__() {}"
        );
        // Should NOT replace inside longer identifiers
        assert_eq!(
            replace_at_word_boundaries("fn getAll() {}", "get", "__E__"),
            "fn getAll() {}"
        );
        assert_eq!(
            replace_at_word_boundaries("fn _get() {}", "get", "__E__"),
            "fn _get() {}"
        );
        // Should replace multiple standalone occurrences
        assert_eq!(
            replace_at_word_boundaries("pub enum Source { Source }", "Source", "__E__"),
            "pub enum __E__ { __E__ }"
        );
        // Should not replace substring at start/end of identifiers
        assert_eq!(
            replace_at_word_boundaries("SourceManager isSource", "Source", "__E__"),
            "SourceManager isSource"
        );
        // Should handle multi-byte UTF-8 characters (emojis) without panicking
        assert_eq!(
            replace_at_word_boundaries("❌ get ✅", "get", "__E__"),
            "❌ __E__ ✅"
        );
        assert_eq!(
            replace_at_word_boundaries("fn 名前() { get }", "get", "__E__"),
            "fn 名前() { __E__ }"
        );
        // Emoji-only content with no needle match should pass through unchanged
        assert_eq!(
            replace_at_word_boundaries("🎉🚀✨", "get", "__E__"),
            "🎉🚀✨"
        );
    }

    #[test]
    fn test_fast_path_identical() {
        let content = "hello world";
        let result = entity_merge(content, content, content, "test.ts");
        assert!(result.is_clean());
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_fast_path_only_ours_changed() {
        let base = "hello";
        let ours = "hello world";
        let result = entity_merge(base, ours, base, "test.ts");
        assert!(result.is_clean());
        assert_eq!(result.content, ours);
    }

    #[test]
    fn test_fast_path_only_theirs_changed() {
        let base = "hello";
        let theirs = "hello world";
        let result = entity_merge(base, base, theirs, "test.ts");
        assert!(result.is_clean());
        assert_eq!(result.content, theirs);
    }

    #[test]
    fn test_different_functions_no_conflict() {
        // Core value prop: two agents add different functions to the same file
        let base = r#"export function existing() {
    return 1;
}
"#;
        let ours = r#"export function existing() {
    return 1;
}

export function agentA() {
    return "added by agent A";
}
"#;
        let theirs = r#"export function existing() {
    return 1;
}

export function agentB() {
    return "added by agent B";
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        assert!(
            result.is_clean(),
            "Should auto-resolve: different functions added. Conflicts: {:?}",
            result.conflicts
        );
        assert!(
            result.content.contains("agentA"),
            "Should contain agentA function"
        );
        assert!(
            result.content.contains("agentB"),
            "Should contain agentB function"
        );
    }

    #[test]
    fn test_same_function_modified_by_both_conflict() {
        let base = r#"export function shared() {
    return "original";
}
"#;
        let ours = r#"export function shared() {
    return "modified by ours";
}
"#;
        let theirs = r#"export function shared() {
    return "modified by theirs";
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        // This should be a conflict since both modified the same function incompatibly
        assert!(
            !result.is_clean(),
            "Should conflict when both modify same function differently"
        );
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].entity_name, "shared");
    }

    #[test]
    fn test_fallback_for_unknown_filetype() {
        // Non-adjacent changes should merge cleanly with line-level merge
        let base = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        let ours = "line 1 modified\nline 2\nline 3\nline 4\nline 5\n";
        let theirs = "line 1\nline 2\nline 3\nline 4\nline 5 modified\n";
        let result = entity_merge(base, ours, theirs, "test.xyz");
        assert!(
            result.is_clean(),
            "Non-adjacent changes should merge cleanly. Conflicts: {:?}",
            result.conflicts,
        );
    }

    #[test]
    fn test_line_level_fallback() {
        // Non-adjacent changes merge cleanly in 3-way merge
        let base = "a\nb\nc\nd\ne\n";
        let ours = "A\nb\nc\nd\ne\n";
        let theirs = "a\nb\nc\nd\nE\n";
        let result = line_level_fallback(base, ours, theirs, "test.rs", &Host::default());
        assert!(result.is_clean());
        assert!(result.stats.used_fallback);
        assert_eq!(result.content, "A\nb\nc\nd\nE\n");
    }

    #[test]
    fn test_line_level_fallback_conflict() {
        // Same line changed differently → conflict
        let base = "a\nb\nc\n";
        let ours = "X\nb\nc\n";
        let theirs = "Y\nb\nc\n";
        let result = line_level_fallback(base, ours, theirs, "test.rs", &Host::default());
        assert!(!result.is_clean());
        assert!(result.stats.used_fallback);
    }

    #[test]
    fn test_expand_separators() {
        let code = "function foo() { return 1; }";
        let expanded = expand_separators(code);
        // Separators are alone on their line, each break carrying its mark.
        let seen: Vec<&str> = expanded.lines().map(str::trim_end).collect();
        assert!(
            seen.iter().any(|l| l.trim_end_matches('\u{1}') == "{"),
            "opening brace should stand alone: {expanded:?}"
        );
        assert!(
            seen.iter().any(|l| l.trim_end_matches('\u{1}') == ";"),
            "semicolon should stand alone: {expanded:?}"
        );
        assert!(
            seen.iter().any(|l| l.trim_end_matches('\u{1}') == "}"),
            "closing brace should stand alone: {expanded:?}"
        );
    }

    #[test]
    fn test_expand_separators_preserves_strings() {
        let code = r#"let x = "hello { world };";"#;
        let expanded = expand_separators(code);
        // Separators inside strings should NOT be expanded
        assert!(
            expanded.contains("\"hello { world };\""),
            "Separators in strings should be preserved: {}",
            expanded
        );
    }

    /// Round-tripping the transform. Everything the fallback path claims rests
    /// on this: the
    /// merge is computed in the expanded world and read back in the original
    /// one, so if the transform is not exactly invertible the merge ships a
    /// reformat nobody asked for. It used to not be — collapse was a no-op.
    #[test]
    fn separator_expansion_is_invertible() {
        for code in [
            "function foo() { return 1; }",
            "use crate::*;\nuse std::fs;\n",
            "buildscript {\n    repositories {\n        jcenter()\n    }\n}\n\nrepositories {\n}\n",
            "class A {\n\tint x = 1;\n\n\tvoid f() {\n\t\tg();\n\t}\n}\n",
            r#"let x = "hello { world };";"#,
            "no separators here at all\n",
            "",
            "trailing blank lines\n\n\n",
            "\n\nleading blank lines\nx = 1;\n",
        ] {
            assert_eq!(
                collapse_separators(&expand_separators(code)),
                code,
                "collapse ∘ expand must be the identity on {code:?}"
            );
        }
    }

    /// A reduced real-world case: one side reorders two `use` lines, the other
    /// appends a third. No entity model reaches a bare `use` list, so this is
    /// the fallback path, and it used to come back with every `;` on a line of
    /// its own — three lines all three versions agreed on, gone from a merge
    /// that exited clean.
    #[test]
    fn reordered_use_statements_keep_their_lines() {
        let base = "use crate::*;\nuse std::fs;\n";
        let ours = "use std::fs;\nuse crate::*;\n";
        let theirs = "use crate::*;\nuse std::fs;\nuse itertools::Itertools;\n";
        let out = line_level_fallback(base, ours, theirs, "x.rs", &Host::default());
        assert!(out.is_clean(), "should still resolve: {:?}", out.content);
        for line in ["use std::fs;", "use crate::*;", "use itertools::Itertools;"] {
            assert!(
                out.content.lines().any(|l| l.trim_end() == line),
                "{line:?} must survive as a line, got {:?}",
                out.content
            );
        }
    }

    #[test]
    fn test_is_import_region() {
        assert!(is_import_region(
            "import foo from 'foo';\nimport bar from 'bar';\n"
        ));
        assert!(is_import_region("use std::io;\nuse std::fs;\n"));
        assert!(!is_import_region("let x = 1;\nlet y = 2;\n"));
        // Mixed: 1 import + 2 non-imports → not import region
        assert!(!is_import_region(
            "import foo from 'foo';\nlet x = 1;\nlet y = 2;\n"
        ));
        // Empty → not import region
        assert!(!is_import_region(""));
    }

    #[test]
    fn test_is_import_line() {
        // JS/TS
        assert!(is_import_line("import foo from 'foo';"));
        assert!(is_import_line("import { bar } from 'bar';"));
        assert!(is_import_line("from typing import List"));
        // Rust
        assert!(is_import_line("use std::io::Read;"));
        // C/C++
        assert!(is_import_line("#include <stdio.h>"));
        // Node require
        assert!(is_import_line("const fs = require('fs');"));
        // Not imports
        assert!(!is_import_line("let x = 1;"));
        assert!(!is_import_line("function foo() {}"));
    }

    #[test]
    fn test_commutative_import_merge_both_add_different() {
        // The key scenario: both branches add different imports
        let base = "import a from 'a';\nimport b from 'b';\n";
        let ours = "import a from 'a';\nimport b from 'b';\nimport c from 'c';\n";
        let theirs = "import a from 'a';\nimport b from 'b';\nimport d from 'd';\n";
        let (result, _order_conflict) = merge_imports_commutatively(base, ours, theirs);
        assert!(result.contains("import a from 'a';"));
        assert!(result.contains("import b from 'b';"));
        assert!(result.contains("import c from 'c';"));
        assert!(result.contains("import d from 'd';"));
    }

    #[test]
    fn test_commutative_import_merge_one_removes() {
        // Ours removes an import, theirs keeps it → removed
        let base = "import a from 'a';\nimport b from 'b';\nimport c from 'c';\n";
        let ours = "import a from 'a';\nimport c from 'c';\n";
        let theirs = "import a from 'a';\nimport b from 'b';\nimport c from 'c';\n";
        let (result, _order_conflict) = merge_imports_commutatively(base, ours, theirs);
        assert!(result.contains("import a from 'a';"));
        assert!(
            !result.contains("import b from 'b';"),
            "Removed import should stay removed"
        );
        assert!(result.contains("import c from 'c';"));
    }

    #[test]
    fn test_commutative_import_merge_both_add_same() {
        // Both add the same import → should appear only once
        let base = "import a from 'a';\n";
        let ours = "import a from 'a';\nimport b from 'b';\n";
        let theirs = "import a from 'a';\nimport b from 'b';\n";
        let (result, _order_conflict) = merge_imports_commutatively(base, ours, theirs);
        let count = result.matches("import b from 'b';").count();
        assert_eq!(count, 1, "Duplicate import should be deduplicated");
    }

    #[test]
    fn test_commutative_import_merge_preserves_file_directive() {
        // Issue #94: // @ts-nocheck on line 1 must stay above imports
        let base = "// @ts-nocheck\nimport { a } from \"./a\";\n";
        let ours = "// @ts-nocheck\nimport { a } from \"./a\";\nimport { b } from \"./b\";\n";
        let theirs = "// @ts-nocheck\nimport { a } from \"./a\";\nimport { c } from \"./c\";\n";
        let (result, _order_conflict) = merge_imports_commutatively(base, ours, theirs);
        assert!(
            result.contains("// @ts-nocheck"),
            "Directive should be preserved. Got: {:?}",
            result
        );
        let nocheck_pos = result.find("// @ts-nocheck").unwrap();
        let first_import_pos = result.find("import").unwrap();
        assert!(
            nocheck_pos < first_import_pos,
            "// @ts-nocheck must stay before imports. Got: {:?}",
            result
        );
        assert!(result.contains("import { b }"), "ours import missing");
        assert!(result.contains("import { c }"), "theirs import missing");
    }

    #[test]
    fn test_commutative_import_merge_preserves_shebang() {
        // Shebang must stay on line 1
        let base = "#!/usr/bin/env node\nimport { a } from \"./a\";\n";
        let ours = "#!/usr/bin/env node\nimport { a } from \"./a\";\nimport { b } from \"./b\";\n";
        let theirs =
            "#!/usr/bin/env node\nimport { a } from \"./a\";\nimport { c } from \"./c\";\n";
        let (result, _order_conflict) = merge_imports_commutatively(base, ours, theirs);
        assert!(
            result.starts_with("#!/usr/bin/env node\n"),
            "Shebang must be first line. Got: {:?}",
            result
        );
    }

    #[test]
    fn test_entity_merge_ts_nocheck_stays_above_imports() {
        // Issue #94 end-to-end: both branches add a different import,
        // // @ts-nocheck must stay on line 1.
        let base = "// @ts-nocheck\nimport { a } from \"./a\";\n\nexport function f() {}\n";
        let ours =
            "// @ts-nocheck\nimport { a } from \"./a\";\nimport { b } from \"./b\";\n\nexport function f() {}\n";
        let theirs =
            "// @ts-nocheck\nimport { a } from \"./a\";\nimport { c } from \"./c\";\n\nexport function f() {}\n";
        let result = entity_merge(base, ours, theirs, "src/example.ts");
        assert!(
            result.is_clean(),
            "Should merge cleanly. Conflicts: {:?}",
            result.conflicts,
        );
        let nocheck_pos = result.content.find("// @ts-nocheck");
        let first_import_pos = result.content.find("import");
        assert!(
            nocheck_pos.is_some(),
            "// @ts-nocheck must be present. Got:\n{}",
            result.content
        );
        assert!(
            nocheck_pos.unwrap() < first_import_pos.unwrap(),
            "// @ts-nocheck must be before imports. Got:\n{}",
            result.content
        );
    }

    #[test]
    fn test_import_preserved_when_both_edit_declaration() {
        // Issue #95: theirs adds an import AND modifies a type, ours also
        // modifies the type. The import must not be dropped.
        let base = "type T = {\n  a: string;\n  b: string;\n};\n";
        let ours = "type T = {\n  a: string;\n  b: string;\n  c: string;\n};\n";
        let theirs = "import { G } from './g';\n\ntype T = {\n  a: G;\n  b: string;\n};\n";
        let result = entity_merge(base, ours, theirs, "test.ts");
        assert!(
            result.content.contains("import { G }"),
            "Import from theirs must be preserved. Got:\n{}",
            result.content
        );
        assert!(
            result.content.contains("c: string"),
            "Field added by ours must be present. Got:\n{}",
            result.content
        );
    }

    #[test]
    fn test_import_preserved_one_sided_entity_change() {
        // One-sided import + entity change should work (fast path).
        // This already works but guard against regressions.
        let base = "type T = {\n  a: string;\n};\n";
        let ours = base;
        let theirs = "import { G } from './g';\n\ntype T = {\n  a: G;\n};\n";
        let result = entity_merge(base, ours, theirs, "test.ts");
        assert!(
            result.content.contains("import { G }"),
            "Import must be preserved in one-sided case. Got:\n{}",
            result.content
        );
    }

    #[test]
    fn test_inner_entity_merge_different_methods() {
        // Two agents modify different methods in the same class
        // This would normally conflict with diffy because the changes are adjacent
        let base = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }

    subtract(a: number, b: number): number {
        return a - b;
    }
}
"#;
        let ours = r#"export class Calculator {
    add(a: number, b: number): number {
        // Added logging
        console.log("adding", a, b);
        return a + b;
    }

    subtract(a: number, b: number): number {
        return a - b;
    }
}
"#;
        let theirs = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }

    subtract(a: number, b: number): number {
        // Added validation
        if (b > a) throw new Error("negative");
        return a - b;
    }
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        assert!(
            result.is_clean(),
            "Different methods modified should auto-merge via inner entity merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(
            result.content.contains("console.log"),
            "Should contain ours changes"
        );
        assert!(
            result.content.contains("negative"),
            "Should contain theirs changes"
        );
    }

    #[test]
    fn test_inner_entity_merge_both_add_different_methods() {
        // Both branches add different methods to the same class
        let base = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }
}
"#;
        let ours = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }

    multiply(a: number, b: number): number {
        return a * b;
    }
}
"#;
        let theirs = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }

    divide(a: number, b: number): number {
        return a / b;
    }
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        assert!(
            result.is_clean(),
            "Both adding different methods should auto-merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(
            result.content.contains("multiply"),
            "Should contain ours's new method"
        );
        assert!(
            result.content.contains("divide"),
            "Should contain theirs's new method"
        );
    }

    #[test]
    fn test_inner_entity_merge_same_method_modified_still_conflicts() {
        // Both modify the same method differently → should still conflict
        let base = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }

    subtract(a: number, b: number): number {
        return a - b;
    }
}
"#;
        let ours = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b + 1;
    }

    subtract(a: number, b: number): number {
        return a - b;
    }
}
"#;
        let theirs = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b + 2;
    }

    subtract(a: number, b: number): number {
        return a - b;
    }
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        assert!(
            !result.is_clean(),
            "Both modifying same method differently should still conflict"
        );
    }

    #[test]
    fn test_extract_member_name() {
        assert_eq!(extract_member_name("add(a, b) {"), "add");
        assert_eq!(extract_member_name("fn add(&self, a: i32) -> i32 {"), "add");
        assert_eq!(extract_member_name("def add(self, a, b):"), "add");
        assert_eq!(
            extract_member_name("public static getValue(): number {"),
            "getValue"
        );
        assert_eq!(extract_member_name("async fetchData() {"), "fetchData");
    }

    #[test]
    fn test_commutative_import_merge_rust_use() {
        let base = "use std::io;\nuse std::fs;\n";
        let ours = "use std::io;\nuse std::fs;\nuse std::path::Path;\n";
        let theirs = "use std::io;\nuse std::fs;\nuse std::collections::HashMap;\n";
        let (result, _order_conflict) = merge_imports_commutatively(base, ours, theirs);
        assert!(result.contains("use std::path::Path;"));
        assert!(result.contains("use std::collections::HashMap;"));
        assert!(result.contains("use std::io;"));
        assert!(result.contains("use std::fs;"));
    }

    #[test]
    fn test_is_whitespace_only_diff_true() {
        // Same content, different indentation
        assert!(is_whitespace_only_diff(
            "    return 1;\n    return 2;\n",
            "      return 1;\n      return 2;\n"
        ));
        // Same content, extra blank lines
        assert!(is_whitespace_only_diff(
            "return 1;\nreturn 2;\n",
            "return 1;\n\nreturn 2;\n"
        ));
    }

    #[test]
    fn test_is_whitespace_only_diff_false() {
        // Different content
        assert!(!is_whitespace_only_diff(
            "    return 1;\n",
            "    return 2;\n"
        ));
        // Added code
        assert!(!is_whitespace_only_diff(
            "return 1;\n",
            "return 1;\nconsole.log('x');\n"
        ));
    }

    #[test]
    fn test_ts_interface_both_add_different_fields() {
        let base = "interface Config {\n    name: string;\n}\n";
        let ours = "interface Config {\n    name: string;\n    age: number;\n}\n";
        let theirs = "interface Config {\n    name: string;\n    email: string;\n}\n";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "TS interface: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content: {:?}", result.content);
        assert!(
            result.is_clean(),
            "Both adding different fields to TS interface should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("age"));
        assert!(result.content.contains("email"));
    }

    #[test]
    fn test_rust_enum_both_add_different_variants() {
        let base = "enum Color {\n    Red,\n    Blue,\n}\n";
        let ours = "enum Color {\n    Red,\n    Blue,\n    Green,\n}\n";
        let theirs = "enum Color {\n    Red,\n    Blue,\n    Yellow,\n}\n";
        let result = entity_merge(base, ours, theirs, "test.rs");
        eprintln!(
            "Rust enum: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content: {:?}", result.content);
        assert!(
            result.is_clean(),
            "Both adding different enum variants should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("Green"));
        assert!(result.content.contains("Yellow"));
    }

    #[test]
    fn test_python_both_add_different_decorators() {
        // Both add different decorators to the same function. Decorator
        // application is function composition — non-commutative — so the
        // stack order of two one-sided additions is a semantic decision
        // neither side made (e.g. @cache outside @auth serves cached
        // responses without an auth check). The merge must conflict, not
        // fabricate an order.
        let base = "def foo():\n    return 1\n\ndef bar():\n    return 2\n";
        let ours = "@cache\ndef foo():\n    return 1\n\ndef bar():\n    return 2\n";
        let theirs = "@deprecated\ndef foo():\n    return 1\n\ndef bar():\n    return 2\n";
        let result = entity_merge(base, ours, theirs, "test.py");
        assert!(
            !result.is_clean(),
            "Both sides adding different decorators must conflict (order is semantic)",
        );
        assert!(result.content.contains("@cache"));
        assert!(result.content.contains("@deprecated"));
    }

    #[test]
    fn test_decorator_plus_body_change() {
        // One adds decorator, other modifies body — should merge both
        let base = "def foo():\n    return 1\n";
        let ours = "@cache\ndef foo():\n    return 1\n";
        let theirs = "def foo():\n    return 42\n";
        let result = entity_merge(base, ours, theirs, "test.py");
        assert!(
            result.is_clean(),
            "Decorator + body change should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("@cache"));
        assert!(result.content.contains("return 42"));
    }

    #[test]
    fn test_ts_class_decorator_merge() {
        // TypeScript decorators on class methods — both add different decorators
        let base = "class Foo {\n    bar() {\n        return 1;\n    }\n}\n";
        let ours = "class Foo {\n    @Injectable()\n    bar() {\n        return 1;\n    }\n}\n";
        let theirs = "class Foo {\n    @Deprecated()\n    bar() {\n        return 1;\n    }\n}\n";
        let result = entity_merge(base, ours, theirs, "test.ts");
        // Non-commutative composition: two one-sided decorator additions
        // must conflict rather than fabricate a stack order neither side
        // wrote.
        assert!(
            !result.is_clean(),
            "Both sides adding different decorators must conflict (order is semantic)",
        );
        assert!(result.content.contains("@Injectable()"));
        assert!(result.content.contains("@Deprecated()"));
    }

    #[test]
    fn test_non_adjacent_intra_function_changes() {
        let base = r#"export function process(data: any) {
    const validated = validate(data);
    const transformed = transform(validated);
    const saved = save(transformed);
    return saved;
}
"#;
        let ours = r#"export function process(data: any) {
    const validated = validate(data);
    const transformed = transform(validated);
    const saved = save(transformed);
    console.log("saved", saved);
    return saved;
}
"#;
        let theirs = r#"export function process(data: any) {
    console.log("input", data);
    const validated = validate(data);
    const transformed = transform(validated);
    const saved = save(transformed);
    return saved;
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        assert!(
            result.is_clean(),
            "Non-adjacent changes within same function should merge via diffy. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("console.log(\"saved\""));
        assert!(result.content.contains("console.log(\"input\""));
    }

    #[test]
    fn test_method_reordering_with_modification() {
        // Agent A reorders methods in class, Agent B modifies one method
        // Inner entity merge matches by name, so reordering should be transparent
        let base = r#"class Service {
    getUser(id: string) {
        return db.find(id);
    }

    createUser(data: any) {
        return db.create(data);
    }

    deleteUser(id: string) {
        return db.delete(id);
    }
}
"#;
        // Ours: reorder methods (move deleteUser before createUser)
        let ours = r#"class Service {
    getUser(id: string) {
        return db.find(id);
    }

    deleteUser(id: string) {
        return db.delete(id);
    }

    createUser(data: any) {
        return db.create(data);
    }
}
"#;
        // Theirs: modify getUser
        let theirs = r#"class Service {
    getUser(id: string) {
        console.log("fetching", id);
        return db.find(id);
    }

    createUser(data: any) {
        return db.create(data);
    }

    deleteUser(id: string) {
        return db.delete(id);
    }
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "Method reorder: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.is_clean(),
            "Method reordering + modification should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(
            result.content.contains("console.log(\"fetching\""),
            "Should contain theirs modification"
        );
        assert!(
            result.content.contains("deleteUser"),
            "Should have deleteUser"
        );
        assert!(
            result.content.contains("createUser"),
            "Should have createUser"
        );
    }

    #[test]
    fn test_doc_comment_plus_body_change() {
        // One side adds JSDoc comment, other modifies function body
        // Doc comments are part of the entity region — they should merge with body changes
        let base = r#"export function calculate(a: number, b: number): number {
    return a + b;
}
"#;
        let ours = r#"/**
 * Calculate the sum of two numbers.
 * @param a - First number
 * @param b - Second number
 */
export function calculate(a: number, b: number): number {
    return a + b;
}
"#;
        let theirs = r#"export function calculate(a: number, b: number): number {
    const result = a + b;
    console.log("result:", result);
    return result;
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "Doc comment + body: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        // This tests whether weave can merge doc comment additions with body changes
    }

    #[test]
    fn test_both_add_different_guard_clauses() {
        // Both add different guard clauses at the start of a function
        let base = r#"export function processOrder(order: Order): Result {
    const total = calculateTotal(order);
    return { success: true, total };
}
"#;
        let ours = r#"export function processOrder(order: Order): Result {
    if (!order) throw new Error("Order required");
    const total = calculateTotal(order);
    return { success: true, total };
}
"#;
        let theirs = r#"export function processOrder(order: Order): Result {
    if (order.items.length === 0) throw new Error("Empty order");
    const total = calculateTotal(order);
    return { success: true, total };
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "Guard clauses: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        // Both add at same position — diffy may struggle since they're at the same insertion point
    }

    #[test]
    fn test_both_modify_different_enum_variants() {
        // One modifies a variant's value, other adds new variants
        let base = r#"enum Status {
    Active = "active",
    Inactive = "inactive",
    Pending = "pending",
}
"#;
        let ours = r#"enum Status {
    Active = "active",
    Inactive = "disabled",
    Pending = "pending",
}
"#;
        let theirs = r#"enum Status {
    Active = "active",
    Inactive = "inactive",
    Pending = "pending",
    Deleted = "deleted",
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "Enum modify+add: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.is_clean(),
            "Modify variant + add new variant should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(
            result.content.contains("\"disabled\""),
            "Should have modified Inactive"
        );
        assert!(
            result.content.contains("Deleted"),
            "Should have new Deleted variant"
        );
    }

    #[test]
    fn test_config_object_field_additions() {
        // Both add different fields to a config object (exported const)
        let base = r#"export const config = {
    timeout: 5000,
    retries: 3,
};
"#;
        let ours = r#"export const config = {
    timeout: 5000,
    retries: 3,
    maxConnections: 10,
};
"#;
        let theirs = r#"export const config = {
    timeout: 5000,
    retries: 3,
    logLevel: "info",
};
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "Config fields: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        // This tests whether inner entity merge handles object literals
        // (it probably won't since object fields aren't extracted as members the same way)
    }

    #[test]
    fn test_call_wrapped_object_scopes_conflict_per_key() {
        // Issue #127: an object literal passed to a call (`configure({ ... })`)
        // closes with `})`, which the container-wrapper detection used to miss,
        // so weave collapsed the whole object into one conflict. It should scope
        // conflicts to the changed keys and leave untouched keys clean, exactly
        // like a bare `{ ... }` object.
        let base =
            "export const flags = configure({\n  a: 1,\n  b: 2,\n  c: 3,\n  d: 4,\n  e: 5,\n});\n";
        let ours =
            "export const flags = configure({\n  a: 10,\n  b: 2,\n  c: 3,\n  d: 4,\n  e: 50,\n});\n";
        let theirs =
            "export const flags = configure({\n  a: 11,\n  b: 2,\n  c: 3,\n  d: 4,\n  e: 51,\n});\n";
        let result = entity_merge(base, ours, theirs, "test.ts");

        // Two scoped conflicts (a and e), not one object-wide conflict.
        let hunks = result.content.matches("<<<<<<<").count();
        assert_eq!(
            hunks, 2,
            "expected per-key conflicts on `a` and `e`, got {hunks}:\n{}",
            result.content
        );
        // Untouched keys stay outside any conflict marker.
        for key in ["  b: 2,", "  c: 3,", "  d: 4,"] {
            assert!(
                result.content.contains(key),
                "untouched key {key:?} should survive cleanly:\n{}",
                result.content
            );
        }
    }

    #[test]
    fn test_rust_impl_block_both_add_methods() {
        // Both add different methods to a Rust impl block
        let base = r#"impl Calculator {
    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
"#;
        let ours = r#"impl Calculator {
    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}
"#;
        let theirs = r#"impl Calculator {
    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    fn divide(&self, a: i32, b: i32) -> i32 {
        a / b
    }
}
"#;
        let result = entity_merge(base, ours, theirs, "test.rs");
        eprintln!(
            "Rust impl: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.is_clean(),
            "Both adding methods to Rust impl should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("multiply"), "Should have multiply");
        assert!(result.content.contains("divide"), "Should have divide");
    }

    #[test]
    fn test_rust_impl_same_trait_different_types() {
        // Two impl blocks for the same trait but different types.
        // Each branch modifies a different impl. Both should be preserved.
        // Regression: sem-core <0.3.10 named both "Stream", causing collision.
        let base = r#"struct Foo;
struct Bar;

impl Stream for Foo {
    type Item = i32;
    fn poll_next(&self) -> Option<i32> {
        Some(1)
    }
}

impl Stream for Bar {
    type Item = String;
    fn poll_next(&self) -> Option<String> {
        Some("hello".into())
    }
}

fn other() {}
"#;
        let ours = r#"struct Foo;
struct Bar;

impl Stream for Foo {
    type Item = i32;
    fn poll_next(&self) -> Option<i32> {
        let x = compute();
        Some(x + 1)
    }
}

impl Stream for Bar {
    type Item = String;
    fn poll_next(&self) -> Option<String> {
        Some("hello".into())
    }
}

fn other() {}
"#;
        let theirs = r#"struct Foo;
struct Bar;

impl Stream for Foo {
    type Item = i32;
    fn poll_next(&self) -> Option<i32> {
        Some(1)
    }
}

impl Stream for Bar {
    type Item = String;
    fn poll_next(&self) -> Option<String> {
        let s = format!("hello {}", name);
        Some(s)
    }
}

fn other() {}
"#;
        let result = entity_merge(base, ours, theirs, "test.rs");
        assert!(
            result.is_clean(),
            "Same trait, different types should not conflict. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(
            result.content.contains("impl Stream for Foo"),
            "Should have Foo impl"
        );
        assert!(
            result.content.contains("impl Stream for Bar"),
            "Should have Bar impl"
        );
        assert!(
            result.content.contains("compute()"),
            "Should have ours' Foo change"
        );
        assert!(
            result.content.contains("format!"),
            "Should have theirs' Bar change"
        );
    }

    #[test]
    fn test_rust_doc_comment_plus_body_change() {
        // One side adds Rust doc comment, other modifies body
        // Comment bundling ensures the doc comment is part of the entity
        let base = r#"fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#;
        let ours = r#"/// Adds two numbers together.
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#;
        let theirs = r#"fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn subtract(a: i32, b: i32) -> i32 {
    a - b - 1
}
"#;
        let result = entity_merge(base, ours, theirs, "test.rs");
        assert!(
            result.is_clean(),
            "Rust doc comment + body change should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(
            result.content.contains("/// Adds two numbers"),
            "Should have ours doc comment"
        );
        assert!(
            result.content.contains("a - b - 1"),
            "Should have theirs body change"
        );
    }

    #[test]
    fn test_both_add_different_doc_comments() {
        // Both add doc comments to different functions — should merge cleanly
        let base = r#"fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#;
        let ours = r#"/// Adds two numbers.
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#;
        let theirs = r#"fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Subtracts b from a.
fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#;
        let result = entity_merge(base, ours, theirs, "test.rs");
        assert!(
            result.is_clean(),
            "Both adding doc comments to different functions should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(
            result.content.contains("/// Adds two numbers"),
            "Should have add's doc comment"
        );
        assert!(
            result.content.contains("/// Subtracts b from a"),
            "Should have subtract's doc comment"
        );
    }

    #[test]
    fn test_go_import_block_both_add_different() {
        // Go uses import (...) blocks — both add different imports
        let base = "package main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n)\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n";
        let ours = "package main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n\t\"strings\"\n)\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n";
        let theirs = "package main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n\t\"io\"\n)\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n";
        let result = entity_merge(base, ours, theirs, "main.go");
        eprintln!(
            "Go import block: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        // This tests whether Go import blocks (a single entity) get inner-merged
    }

    #[test]
    fn test_python_class_both_add_methods() {
        // Python class — both add different methods
        let base = "class Calculator:\n    def add(self, a, b):\n        return a + b\n";
        let ours = "class Calculator:\n    def add(self, a, b):\n        return a + b\n\n    def multiply(self, a, b):\n        return a * b\n";
        let theirs = "class Calculator:\n    def add(self, a, b):\n        return a + b\n\n    def divide(self, a, b):\n        return a / b\n";
        let result = entity_merge(base, ours, theirs, "test.py");
        eprintln!(
            "Python class: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.is_clean(),
            "Both adding methods to Python class should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("multiply"), "Should have multiply");
        assert!(result.content.contains("divide"), "Should have divide");
    }

    #[test]
    fn test_interstitial_conflict_not_silently_embedded() {
        // Regression test: when interstitial content between entities has a
        // both-modified conflict, merge_interstitials must report it as a real
        // conflict instead of silently embedding raw diffy markers and claiming
        // is_clean=true.
        //
        // Scenario: a barrel export file (index.ts) with comments between
        // export statements. Both sides modify the SAME interstitial comment
        // block differently. The exports are the entities; the comment between
        // them is interstitial content that goes through merge_interstitials
        // → diffy, which cannot auto-merge conflicting edits.
        let base = r#"export { alpha } from "./alpha";

// Section: data utilities
// TODO: add more exports here

export { beta } from "./beta";
"#;
        let ours = r#"export { alpha } from "./alpha";

// Section: data utilities (sorting)
// Sorting helpers for list views

export { beta } from "./beta";
"#;
        let theirs = r#"export { alpha } from "./alpha";

// Section: data utilities (filtering)
// Filtering helpers for search views

export { beta } from "./beta";
"#;
        let result = entity_merge(base, ours, theirs, "index.ts");

        // The key assertions:
        // 1. If the content has conflict markers, is_clean() MUST be false
        let has_markers = result.content.contains("<<<<<<<") || result.content.contains(">>>>>>>");
        if has_markers {
            assert!(
                !result.is_clean(),
                "BUG: is_clean()=true but merged content has conflict markers!\n\
                 stats: {}\nconflicts: {:?}\ncontent:\n{}",
                result.stats,
                result.conflicts,
                result.content
            );
            assert!(
                result.stats.entities_conflicted > 0,
                "entities_conflicted should be > 0 when markers are present"
            );
        }

        // 2. If it was resolved cleanly, no markers should exist
        if result.is_clean() {
            assert!(
                !has_markers,
                "Clean merge should not contain conflict markers!\ncontent:\n{}",
                result.content
            );
        }
    }

    #[test]
    fn test_pre_conflicted_input_not_treated_as_clean() {
        // Regression test for AU/AA conflicts: git can store conflict markers
        // directly into stage blobs. Weave must not return is_clean=true.
        let base = "";
        let theirs = "";
        let ours = r#"/**
 * MIT License
 */

<<<<<<<< HEAD:src/lib/exports/index.ts
export { renderDocToBuffer } from "./doc-exporter";
export type { ExportOptions, ExportMetadata, RenderContext } from "./types";
========
export * from "./editor";
export * from "./types";
>>>>>>>> feature:packages/core/src/editor/index.ts
"#;
        let result = entity_merge(base, ours, theirs, "index.ts");

        assert!(
            !result.is_clean(),
            "Pre-conflicted input must not be reported as clean!\n\
             stats: {}\nconflicts: {:?}",
            result.stats,
            result.conflicts,
        );
        assert!(result.stats.entities_conflicted > 0);
        assert!(!result.conflicts.is_empty());
    }

    #[test]
    fn test_multi_line_signature_classified_as_syntax() {
        // Multi-line parameter list: changing a param should be Syntax, not Functional
        let base = "function process(\n    a: number,\n    b: string\n) {\n    return a;\n}\n";
        let ours = "function process(\n    a: number,\n    b: string,\n    c: boolean\n) {\n    return a;\n}\n";
        let theirs = "function process(\n    a: number,\n    b: number\n) {\n    return a;\n}\n";
        let complexity = crate::conflict::classify_conflict(Some(base), Some(ours), Some(theirs));
        assert_eq!(
            complexity,
            crate::conflict::ConflictComplexity::Syntax,
            "Multi-line signature change should be classified as Syntax, got {:?}",
            complexity
        );
    }

    #[test]
    fn test_grouped_import_merge_preserves_groups() {
        let base = "import os\nimport sys\n\nfrom collections import OrderedDict\nfrom typing import List\n";
        let ours = "import os\nimport sys\nimport json\n\nfrom collections import OrderedDict\nfrom typing import List\n";
        let theirs = "import os\nimport sys\n\nfrom collections import OrderedDict\nfrom collections import defaultdict\nfrom typing import List\n";
        let (result, _order_conflict) = merge_imports_commutatively(base, ours, theirs);
        // json should be in the first group (stdlib), defaultdict in the second (collections)
        let lines: Vec<&str> = result.lines().collect();
        let json_idx = lines.iter().position(|l| l.contains("json"));
        let blank_idx = lines.iter().position(|l| l.trim().is_empty());
        let defaultdict_idx = lines.iter().position(|l| l.contains("defaultdict"));
        assert!(json_idx.is_some(), "json import should be present");
        assert!(
            blank_idx.is_some(),
            "blank line separator should be present"
        );
        assert!(
            defaultdict_idx.is_some(),
            "defaultdict import should be present"
        );
        // json should come before the blank line, defaultdict after
        assert!(
            json_idx.unwrap() < blank_idx.unwrap(),
            "json should be in first group"
        );
        assert!(
            defaultdict_idx.unwrap() > blank_idx.unwrap(),
            "defaultdict should be in second group"
        );
    }

    #[test]
    fn test_configurable_duplicate_threshold() {
        // Create entities with 15 same-name entities
        let entities: Vec<SemanticEntity> = (0..15)
            .map(|i| SemanticEntity {
                id: format!("test::function::test_{}", i),
                file_path: "test.ts".to_string(),
                entity_type: "function".to_string(),
                name: "test".to_string(),
                parent_id: None,
                content: format!("function test() {{ return {}; }}", i),
                content_hash: format!("hash_{}", i),
                structural_hash: None,
                kappa: None,
                start_line: i * 3 + 1,
                end_line: i * 3 + 3,
                start_byte: None,
                end_byte: None,
                metadata: None,
            })
            .collect();
        // The default host's threshold (10): should trigger.
        assert!(has_excessive_duplicates(
            &entities,
            Host::default().max_duplicates
        ));
        // A host granting a threshold of 20: should not. No environment is
        // read, and no other test in this process can see the change.
        assert!(!has_excessive_duplicates(&entities, 20));
    }

    #[test]
    fn test_ts_multiline_import_consolidation() {
        // Issue #24: when incoming consolidates two imports into one multi-line import,
        // the `import {` opening line can get dropped.
        let base = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        let ours = base;
        let theirs = "\
import {
     type Foo,
     type a,
     type b,
     type c,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "TS import consolidation: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        // Theirs is the only change, result should match theirs exactly
        assert!(
            result.content.contains("import {"),
            "import {{ must not be dropped"
        );
        assert!(
            result.content.contains("type Foo,"),
            "type Foo must be present"
        );
        assert!(
            result.content.contains("} from \"./foo\""),
            "closing must be present"
        );
        assert!(
            !result.content.contains("import type { Foo }"),
            "old separate import should be removed"
        );
    }

    #[test]
    fn test_ts_multiline_import_both_modify() {
        // Issue #24 variant: both sides modify the import block
        let base = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        // Ours: consolidates imports + adds type d
        let ours = "\
import {
     type Foo,
     type a,
     type b,
     type c,
     type d,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        // Theirs: consolidates imports + adds type e
        let theirs = "\
import {
     type Foo,
     type a,
     type b,
     type c,
     type e,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "TS import both modify: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.content.contains("import {"),
            "import {{ must not be dropped"
        );
        assert!(
            result.content.contains("type Foo,"),
            "type Foo must be present"
        );
        assert!(
            result.content.contains("type d,"),
            "ours addition must be present"
        );
        assert!(
            result.content.contains("type e,"),
            "theirs addition must be present"
        );
        assert!(
            result.content.contains("} from \"./foo\""),
            "closing must be present"
        );
    }

    #[test]
    fn test_ts_multiline_import_no_entities() {
        // Issue #24: file with only imports, no other entities
        let base = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
} from \"./foo\"
";
        let ours = base;
        let theirs = "\
import {
     type Foo,
     type a,
     type b,
     type c,
} from \"./foo\"
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "TS import no entities: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.content.contains("import {"),
            "import {{ must not be dropped"
        );
        assert!(
            result.content.contains("type Foo,"),
            "type Foo must be present"
        );
    }

    #[test]
    fn test_ts_multiline_import_export_variable() {
        // Issue #24: import block near an export variable entity
        let base = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
} from \"./foo\"

export const X = 1;

export function bar() {
    return 1;
}
";
        let ours = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
     type d,
} from \"./foo\"

export const X = 1;

export function bar() {
    return 1;
}
";
        let theirs = "\
import {
     type Foo,
     type a,
     type b,
     type c,
} from \"./foo\"

export const X = 2;

export function bar() {
    return 1;
}
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "TS import + export var: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.content.contains("import {"),
            "import {{ must not be dropped"
        );
    }

    #[test]
    fn test_ts_multiline_import_adjacent_to_entity() {
        // Issue #24: import block directly adjacent to entity (no blank line)
        let base = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
} from \"./foo\"
export function bar() {
    return 1;
}
";
        let ours = base;
        let theirs = "\
import {
     type Foo,
     type a,
     type b,
     type c,
} from \"./foo\"
export function bar() {
    return 1;
}
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "TS import adjacent: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.content.contains("import {"),
            "import {{ must not be dropped"
        );
        assert!(
            result.content.contains("type Foo,"),
            "type Foo must be present"
        );
    }

    #[test]
    fn test_ts_multiline_import_both_consolidate_differently() {
        // Issue #24: both sides consolidate imports but add different specifiers
        let base = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        let ours = "\
import {
     type Foo,
     type a,
     type b,
     type c,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        let theirs = "\
import {
     type Foo,
     type a,
     type b,
     type d,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "TS both consolidate: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.content.contains("import {"),
            "import {{ must not be dropped"
        );
        assert!(
            result.content.contains("type Foo,"),
            "type Foo must be present"
        );
        assert!(
            result.content.contains("} from \"./foo\""),
            "closing must be present"
        );
    }

    #[test]
    fn test_ts_multiline_import_ours_adds_theirs_consolidates() {
        // Issue #24 variant: ours adds new import, theirs consolidates
        let base = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        // Ours: adds a new specifier to the multiline import
        let ours = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
     type d,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        // Theirs: consolidates into one import
        let theirs = "\
import {
     type Foo,
     type a,
     type b,
     type c,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "TS import ours-adds theirs-consolidates: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.content.contains("import {"),
            "import {{ must not be dropped"
        );
        assert!(
            result.content.contains("type d,"),
            "ours addition must be present"
        );
        assert!(
            result.content.contains("} from \"./foo\""),
            "closing must be present"
        );
    }

    #[test]
    fn test_ts_multiline_import_multiple_sources_no_closing_leak() {
        // Issue #24 latest repro: multiple multi-line imports from different sources.
        // The `} from "..."` closing lines were leaking into the non-import diffy merge,
        // producing orphaned `} from` lines without their `import {` opening lines.
        let base = "\
import {
    type A,
} from \"./file1\"
import {
    type B,
} from \"./file2\"
import {
    type C,
} from \"./file3\"

export function main() { return 1; }
";
        // Ours: adds a specifier to file1 and a new import from file4
        let ours = "\
import {
    type A,
    type A2,
} from \"./file1\"
import {
    type B,
} from \"./file2\"
import {
    type C,
} from \"./file3\"
import {
    type D,
} from \"./file4\"

export function main() { return 1; }
";
        // Theirs: adds a specifier to file3
        let theirs = "\
import {
    type A,
} from \"./file1\"
import {
    type B,
} from \"./file2\"
import {
    type C,
    type C2,
} from \"./file3\"

export function main() { return 1; }
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!("Multiple source imports: clean={}", result.is_clean());
        eprintln!("Content:\n{}", result.content);

        // All specifiers should be present
        assert!(result.content.contains("type A,"), "A must be present");
        assert!(
            result.content.contains("type A2,"),
            "A2 (ours addition) must be present"
        );
        assert!(result.content.contains("type B,"), "B must be present");
        assert!(result.content.contains("type C,"), "C must be present");
        assert!(
            result.content.contains("type C2,"),
            "C2 (theirs addition) must be present"
        );
        assert!(
            result.content.contains("type D,"),
            "D (ours new import) must be present"
        );

        // Count `import {` vs `} from` — they must be balanced
        let open_count = result
            .content
            .lines()
            .filter(|l| l.trim().starts_with("import {"))
            .count();
        let close_count = result
            .content
            .lines()
            .filter(|l| {
                let t = l.trim();
                t.starts_with('}') && t.contains("from ")
            })
            .count();
        assert_eq!(
            open_count, close_count,
            "import {{ and }} from must be balanced: {} opens vs {} closes\n{}",
            open_count, close_count, result.content
        );
    }

    #[test]
    fn test_rename_plus_modify_auto_resolves() {
        // Issue #53: ours renames a variable (IDE rename symbol), theirs modifies it.
        // Should detect the rename via token similarity and merge cleanly.
        let base = r#"export const cubeQueryExecutorTool = tool({
    name: "cubeQueryExecutorTool",
    description: "Execute a cube query",
    schema: z.object({ query: z.string() }),
    execute: async (input) => {
        return await runQuery(input.query);
    },
});
"#;
        // Ours: renamed cubeQueryExecutorTool → cubeQueryTool (all refs updated)
        let ours = r#"export const cubeQueryTool = tool({
    name: "cubeQueryTool",
    description: "Execute a cube query",
    schema: z.object({ query: z.string() }),
    execute: async (input) => {
        return await runQuery(input.query);
    },
});
"#;
        // Theirs: modified the body (added unit inference logic)
        let theirs = r#"export const cubeQueryExecutorTool = tool({
    name: "cubeQueryExecutorTool",
    description: "Execute a cube query with unit inference",
    schema: z.object({ query: z.string(), unit: z.string().optional() }),
    execute: async (input) => {
        const unit = input.unit || inferUnit(input.query);
        return await runQuery(input.query, unit);
    },
});
"#;
        let result = entity_merge(base, ours, theirs, "cubeQueryTool.ts");
        // Rename + modify should conflict (developer who modified didn't know about rename)
        assert_eq!(
            result.conflicts.len(),
            1,
            "Should have exactly one conflict"
        );
        assert!(
            matches!(
                result.conflicts[0].kind,
                ConflictKind::RenameModify {
                    renamed_in_ours: true,
                    ..
                }
            ),
            "Should be a RenameModify conflict, got: {:?}",
            result.conflicts[0].kind
        );
        // Both versions should be present in the conflict
        assert!(
            result.content.contains("cubeQueryTool"),
            "Ours (renamed) should be in conflict markers"
        );
        assert!(
            result.content.contains("unit inference"),
            "Theirs (modified) should be in conflict markers"
        );
    }
}
