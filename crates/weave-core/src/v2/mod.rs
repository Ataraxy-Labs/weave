//! weave-core v2: the merge as a typed pipeline.
//!
//! ```text
//!   parse ─▶ Match ─▶ Classify ─▶ Resolve ─▶ Bind ─▶ Plan ─▶ Render
//!            │         │           │          │       │       │
//!            │         │           │          │       │       └ a fold; decides nothing
//!            │         │           │          │       └ ordering only
//!            │         │           │          └ may turn a disposition into a conflict
//!            │         │           └ Cell → Disposition, pure
//!            │         └ Triple → Cell, pure
//!            └ the ONLY place entities are paired; every entity lands in one triple
//! ```
//!
//! Each stage receives only the inputs it needs to do its job, and every fact has exactly
//! one owner. Things v1 defended with guards — no duplication, no double
//! resolution, no direction dependence, termination — the shape of these types
//! takes care of instead:
//!
//! | v1 guard | replaced by |
//! |---|---|
//! | `MAX_RENAME_BUCKET` | indexed candidate generation; equal-evidence buckets pair in key order |
//! | merge timeout thread | no pairwise scan anywhere in matching |
//! | `resolved_entities` keyed by three aliases | one triple owns one entity's three claims |
//! | `theirs_rename_base_ids` | claims are linear: an entity cannot be emitted twice |
//! | `post_merge_cleanup` | render is a fold over a plan, so it cannot fabricate duplicates |
//! | marker-scanning `is_clean` | typed conflicts are the only source of truth |

pub mod bind;
pub mod classify;
pub mod match_phase;
pub mod plan;
pub mod render;
pub mod resolve;
pub mod types;

use std::borrow::Cow;
use std::collections::HashMap;

use sem_core::parser::registry::ParserRegistry;

pub(crate) use classify::classify;
pub(crate) use match_phase::match_phase;
pub(crate) use match_phase::{build_arena, RawEntity};
pub use types::*;

use crate::conflict::{EntityConflict, MarkerFormat, MergeStats};
use crate::host::Host;
use crate::merge::{EntityAudit, MergeResult};
use crate::region::{extract_regions, FileRegion};
use crate::validate::{SemanticWarning, WarningKind};

/// Normalize encoding at the parse boundary: strip a UTF-8 BOM and fold CRLF
/// to LF.
///
/// Without this, one side running a line-ending or BOM normalizer makes *every*
/// entity's content differ from base, so every key lands in an EDIT×EDIT cell
/// and a whole-file mass conflict follows from a change that means nothing.
/// Doing it here — once, at the only
/// place text enters the pipeline — is what makes it impossible for a later
/// stage to compare un-normalized text by accident.
pub(crate) fn normalize_encoding(input: &str) -> Cow<'_, str> {
    let stripped = input.strip_prefix('\u{feff}').unwrap_or(input);
    if stripped.contains('\r') {
        Cow::Owned(stripped.replace("\r\n", "\n"))
    } else if stripped.len() == input.len() {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(stripped.to_string())
    }
}

/// Did normalization change anything? Used to report the mass-edit hazard
/// rather than silently absorbing it.
/// Test-only: the encoding decision is made by `Encoding::of`, which the
/// pipeline consults directly. This predicate exists so the tests can state the
/// same question in one line.
#[cfg(test)]
pub(crate) fn encoding_differs(base: &str, ours: &str, theirs: &str) -> bool {
    let raw_crlf = |s: &str| s.contains("\r\n");
    let bom = |s: &str| s.starts_with('\u{feff}');
    let crlf = [raw_crlf(base), raw_crlf(ours), raw_crlf(theirs)];
    let boms = [bom(base), bom(ours), bom(theirs)];
    crlf.iter().any(|x| *x) && !crlf.iter().all(|x| *x)
        || boms.iter().any(|x| *x) && !boms.iter().all(|x| *x)
}

/// Read one version into [`RawEntity`]s, using region content (the file's own
/// bytes, including leading doc comments) as the authoritative text.
pub(crate) fn read_version(
    content: &str,
    file_path: &str,
    registry: &ParserRegistry,
) -> Option<Vec<RawEntity>> {
    let plugin = registry
        .get_plugin(file_path)
        .filter(|p| p.id() != "fallback")?;
    let all = plugin.extract_entities(content, file_path);
    let top = crate::merge::filter_nested_entities(all);
    let regions = extract_regions(content, &top);
    Some(raws(&top, &regions, entity_separator(file_path)))
}

/// Match + classify a real three-way merge, and stop there.
///
/// The first two stages of [`merge_file`], exposed on their own so a test can
/// ask what the pipeline *decided* without asking it to render anything. Both
/// go through the same [`raws`] pairing, so the two callers cannot end up
/// disagreeing about what an entity's text is.
pub(crate) fn analyze(
    base: &str,
    ours: &str,
    theirs: &str,
    file_path: &str,
    registry: &ParserRegistry,
) -> Option<Analysis> {
    let base = normalize_encoding(base);
    let ours = normalize_encoding(ours);
    let theirs = normalize_encoding(theirs);
    let b = read_version(&base, file_path, registry)?;
    let o = read_version(&ours, file_path, registry)?;
    let t = read_version(&theirs, file_path, registry)?;
    let (arena, claims) = build_arena(b, o, t);
    let matching = match_phase(arena, claims);
    let cells = matching
        .triples
        .iter()
        .map(|tr| classify(&matching.arena, tr))
        .collect();
    Some(Analysis { matching, cells })
}

/// [`analyze`] against the process-wide parser registry.
pub fn analyze_default(base: &str, ours: &str, theirs: &str, file_path: &str) -> Option<Analysis> {
    analyze(
        base,
        ours,
        theirs,
        file_path,
        &crate::merge::PARSER_REGISTRY,
    )
}

/// A matched, classified merge — the input to every later stage.
#[derive(Debug)]
pub struct Analysis {
    pub matching: Matching,
    /// Parallel to `matching.triples`.
    pub cells: Vec<Cell>,
}

impl Analysis {
    pub fn iter(&self) -> impl Iterator<Item = (&Triple, Cell)> {
        self.matching.triples.iter().zip(self.cells.iter().copied())
    }
    /// The name a triple travels under: the base name when there is one (both
    /// sides agree on it), else the side that has the entity.
    pub fn label(&self, triple: &Triple) -> String {
        self.matching
            .arena
            .get(triple.representative())
            .name()
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_and_bom_normalize_to_one_shape() {
        assert_eq!(normalize_encoding("a\r\nb\r\n"), "a\nb\n");
        assert_eq!(normalize_encoding("\u{feff}a\nb\n"), "a\nb\n");
        assert_eq!(normalize_encoding("\u{feff}a\r\nb\r\n"), "a\nb\n");
        assert_eq!(normalize_encoding("a\nb\n"), "a\nb\n");
    }

    #[test]
    fn a_one_sided_crlf_conversion_is_reported_as_an_encoding_change() {
        assert!(encoding_differs("a\n", "a\r\n", "a\n"));
        assert!(!encoding_differs("a\n", "a\n", "a\n"));
        assert!(!encoding_differs("a\r\n", "a\r\n", "a\r\n"));
    }

    /// The summary is a derived read of the dispositions, so it cannot
    /// disagree with them. Two things follow, and neither was guaranteed while
    /// `MergeStats` was a shared `&mut` counter that `bind` had to repair with
    /// `saturating_sub`:
    ///
    ///   * every entity is counted on exactly one line — no double count from a
    ///     revision that added without subtracting, no lost count from a
    ///     `saturating_sub` that clamped at zero;
    ///   * `stats.has_conflicts()` and `is_clean()` answer the same question,
    ///     though they read different fields.
    #[test]
    fn the_summary_cannot_disagree_with_the_decisions_it_summarises() {
        // (base, ours, theirs) chosen to exercise the revising passes: a plain
        // divergent edit, a rename whose call sites need repair, a deletion the
        // other side's new code depends on, and a shadowing addition.
        //
        // None of them provoke an interstitial or definition-order conflict,
        // which are counted in `entities_conflicted` without being entities —
        // so here the entity count and the audit length are exactly equal.
        let cases: [(&str, &str, &str); 4] = [
            (
                "def a():\n    return 1\n",
                "def a():\n    return 2\n",
                "def a():\n    return 3\n",
            ),
            (
                "def load(i):\n    return i\n\n\ndef use():\n    return load(1)\n",
                "def fetch(i):\n    return i\n\n\ndef use():\n    return load(1)\n",
                "def load(i):\n    return i\n\n\ndef use():\n    return load(2)\n",
            ),
            (
                "def gone():\n    return 1\n",
                "",
                "def gone():\n    return 1\n\n\ndef caller():\n    return gone()\n",
            ),
            (
                "def keep():\n    return 1\n",
                "def keep():\n    return 1\n\n\ndef helper():\n    return 9\n",
                "def keep():\n    return 1\n\n\ndef other():\n    return helper()\n",
            ),
        ];
        for (base, ours, theirs) in cases {
            let Ok(r) = merge_file(
                base,
                ours,
                theirs,
                "m.py",
                &crate::merge::PARSER_REGISTRY,
                &MarkerFormat::default(),
                &Host::default(),
            ) else {
                continue;
            };
            let s = &r.stats;
            let counted = s.entities_unchanged
                + s.entities_ours_only
                + s.entities_theirs_only
                + s.entities_both_changed_merged
                + s.entities_added_ours
                + s.entities_added_theirs
                + s.entities_deleted
                + s.entities_conflicted;
            assert_eq!(
                counted,
                r.audit.len(),
                "every entity is counted exactly once\nbase: {base:?}\nstats: {s:?}"
            );
            assert_eq!(
                s.has_conflicts(),
                !r.is_clean(),
                "the counter and the typed conflicts disagree\nbase: {base:?}"
            );
        }
    }

    /// Reproduces #148: a side that deletes every entity in the file
    /// collapses, in `extract_regions`, to a single `file_only` region
    /// instead of the `file_header`/`file_footer` keys the other two sides
    /// keep for the very same trailing text. `merge_interstitials` 3-way
    /// merges by key, so on that side `file_footer` simply is not present
    /// and reads back as `""`; since base and theirs still agree on
    /// `file_footer`, the ladder's second rung (`base == theirs -> take
    /// ours`) picks that empty string, discarding text every one of the
    /// three inputs agreed on.
    ///
    /// A single-entity repro would not show this: `render` has a *separate*,
    /// correct fallback that emits `file_only` verbatim when the whole
    /// merge produces zero surviving items, which papers over the bug. This
    /// case keeps `bar` alive as a genuine modify/delete conflict so `items`
    /// is non-empty and that fallback cannot fire.
    #[test]
    fn shared_top_level_text_survives_when_one_side_deletes_every_entity() {
        let base = "def foo():\n    return 1\n\n\ndef bar():\n    return 1\n\n\nshared = 5\n";
        let ours = "shared = 5\n";
        let theirs = "def foo():\n    return 1\n\n\ndef bar():\n    return 2\n\n\nshared = 5\n";

        let result = merge_file(
            base,
            ours,
            theirs,
            "m.py",
            &crate::merge::PARSER_REGISTRY,
            &MarkerFormat::default(),
            &Host::default(),
        )
        .expect("theirs still has entities; this must not fall back to Unsupported");

        assert!(
            result.content.contains("shared = 5"),
            "top-level statement unanimous across base/ours/theirs must survive \
             the merge, got: {:?}",
            result.content,
        );
    }
}

// ---------------------------------------------------------------------------
// The pipeline
// ---------------------------------------------------------------------------

/// Why v2 handed a file back to the line-level path. Each variant is a
/// construct the entity model does not describe, not a bug it is dodging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unsupported {
    /// No grammar for this file type (or only the 20-line chunk fallback,
    /// whose chunk boundaries are not structural boundaries).
    NoGrammar,
    /// A non-empty file the grammar found no entities in: there is no entity
    /// layer to merge at, so the guarantee degrades to exactly diff3.
    NoEntities,
    /// Base is empty and both sides wrote the file: entity reconstruction of a
    /// structured format (JSON) can place content after a closing delimiter.
    BothCreated,
    /// So many same-named entities that identity is not a function. The v2
    /// matcher terminates regardless; the *cell verdicts* would be fiction.
    AmbiguousIdentity,
}

/// Run the full pipeline. `Err(Unsupported)` means the caller should use the
/// line-level path — the file is outside the entity model, not outside v2.
pub(crate) fn merge_file(
    base: &str,
    ours: &str,
    theirs: &str,
    file_path: &str,
    registry: &ParserRegistry,
    marker_format: &MarkerFormat,
    host: &Host,
) -> Result<MergeResult, Unsupported> {
    let Some(plugin) = registry
        .get_plugin(file_path)
        .filter(|p| p.id() != "fallback")
    else {
        return Err(Unsupported::NoGrammar);
    };
    // Encoding normalization, once, at the only door into the pipeline.
    // A side that ran a CRLF or BOM normalizer would otherwise
    // make every entity's content differ from base, turning a change that means
    // nothing into a whole-file mass conflict.
    //
    // Normalizing is not the same as *deciding* the file's encoding: if all
    // three inputs agree on CRLF or on a BOM, the merge has no mandate to
    // change it, and the shape is restored on the way out. Normalization exists
    // so the comparison is meaningful, not so the output is opinionated.
    let restore = Encoding::of(base, ours, theirs);
    let base = &*normalize_encoding(base);
    let ours = &*normalize_encoding(ours);
    let theirs = &*normalize_encoding(theirs);
    if base.trim().is_empty() && !ours.trim().is_empty() && !theirs.trim().is_empty() {
        return Err(Unsupported::BothCreated);
    }

    let base_all = plugin.extract_entities(base, file_path);
    let ours_all = plugin.extract_entities(ours, file_path);
    let theirs_all = plugin.extract_entities(theirs, file_path);
    let base_top = crate::merge::filter_nested_entities(base_all.clone());
    let ours_top = crate::merge::filter_nested_entities(ours_all.clone());
    let theirs_top = crate::merge::filter_nested_entities(theirs_all.clone());

    if base_top.is_empty() && !base.trim().is_empty() {
        return Err(Unsupported::NoEntities);
    }
    if ours_top.is_empty()
        && !ours.trim().is_empty()
        && theirs_top.is_empty()
        && !theirs.trim().is_empty()
    {
        return Err(Unsupported::NoEntities);
    }
    let max_duplicates = host.max_duplicates;
    if crate::merge::has_excessive_duplicates(&base_top, max_duplicates)
        || crate::merge::has_excessive_duplicates(&ours_top, max_duplicates)
        || crate::merge::has_excessive_duplicates(&theirs_top, max_duplicates)
    {
        return Err(Unsupported::AmbiguousIdentity);
    }

    let base_regions = extract_regions(base, &base_top);
    let ours_regions = extract_regions(ours, &ours_top);
    let theirs_regions = extract_regions(theirs, &theirs_top);

    // -- Match, Classify -------------------------------------------------
    // The sequence separator comes off here and goes back on at render: every
    // stage between the two sees the entity, not its position in the object.
    let separator = entity_separator(file_path);
    let (arena, claims) = build_arena(
        raws(&base_top, &base_regions, separator),
        raws(&ours_top, &ours_regions, separator),
        raws(&theirs_top, &theirs_regions, separator),
    );
    let matching = match_phase(arena, claims);
    let cells: Vec<Cell> = matching
        .triples
        .iter()
        .map(|t| classify(&matching.arena, t))
        .collect();

    // -- Resolve ---------------------------------------------------------
    let p = file_path.to_ascii_lowercase();
    let ctx = resolve::ResolveCtx {
        marker_format,
        indent_sensitive: p.ends_with(".py") || p.ends_with(".yaml") || p.ends_with(".yml"),
        decorators_compose: [".py", ".pyi", ".ts", ".tsx", ".js", ".jsx", ".mjs"]
            .iter()
            .any(|e| p.ends_with(e)),
        base_all: &base_all,
        ours_all: &ours_all,
        theirs_all: &theirs_all,
        host,
    };
    let mut resolved: Vec<resolve::Resolved> = matching
        .triples
        .iter()
        .zip(cells.iter())
        .map(|(t, c)| resolve::resolve(&matching.arena, t, *c, &ctx))
        .collect();

    // -- Bind ------------------------------------------------------------
    let bind_ctx = bind::BindCtx {
        marker_format,
        file_path,
        resolve: &ctx,
    };
    let bind_report = bind::bind(
        &matching.arena,
        &matching.triples,
        &cells,
        &matching.relational,
        &mut resolved,
        &bind_ctx,
    );

    // The summary is READ off the finished decisions, once, here. No stage
    // upstream holds a handle to it, so none of them can leave it disagreeing
    // with what was actually decided.
    let mut stats = summarize(&resolved);
    stats.references_rewritten = bind_report.references_rewritten;

    // -- Plan, Render ----------------------------------------------------
    let (merged_interstitials, interstitial_conflicts) = crate::merge::merge_interstitials(
        &base_regions,
        &ours_regions,
        &theirs_regions,
        marker_format,
    );
    let placement = plan::plan(
        &matching.arena,
        &matching.triples,
        &resolved,
        EntityOrder::of_path(file_path),
    );
    let emitted = render::emitted_src_ids(
        &matching.arena,
        &matching.triples,
        &resolved,
        &placement.items,
    );
    let interstitials = render::Interstitials::new(
        &merged_interstitials,
        &[&base_regions, &ours_regions, &theirs_regions],
        &emitted,
    );
    let mut content = render::render(
        &matching.arena,
        &matching.triples,
        &resolved,
        &placement.items,
        &interstitials,
        separator,
    );

    // -- The result is read off the dispositions, not off the text -------
    let mut conflicts: Vec<EntityConflict> = resolved
        .iter()
        .filter_map(|r| match &r.disposition {
            Disposition::Conflict { conflict, .. } => Some(conflict.clone()),
            _ => None,
        })
        .collect();
    stats.entities_conflicted += interstitial_conflicts.len();
    conflicts.extend(interstitial_conflicts);

    // The ORDER rule at the entity layer. Both sides reordered the
    // definitions of a file whose language evaluates them in order, and they
    // disagree about entities they SHARE — so no output sequence is both
    // sides' program. The entities themselves merged fine, so they are all
    // still emitted, once each; the undecidable part is stated in markers at
    // the head of the file rather than silently resolved by picking a side.
    if let Some(oc) = &placement.order_conflict {
        // One line per side, not one line per name: a per-name listing shares a
        // common suffix with the other side's, and the marker renderer would
        // hoist it OUTSIDE the markers as if it were agreed source text.
        let listing =
            |names: &[String]| Some(format!("definition order: {}\n", names.join(" -> ")));
        let conflict = EntityConflict {
            entity_name: "definition order".to_string(),
            entity_type: "entity-order".to_string(),
            kind: crate::conflict::ConflictKind::BothModified,
            complexity: crate::conflict::ConflictComplexity::SyntaxFunctional,
            ours_content: listing(&oc.ours),
            theirs_content: listing(&oc.theirs),
            base_content: listing(&oc.base),
        };
        content = format!(
            "{}{}",
            conflict.to_conflict_markers(marker_format, "merge_ladder_exhausted"),
            content
        );
        stats.entities_conflicted += 1;
        conflicts.push(conflict);
    }
    let audit: Vec<EntityAudit> = matching
        .triples
        .iter()
        .zip(resolved.iter())
        .map(|(t, r)| {
            let e = matching.arena.get(t.representative());
            EntityAudit {
                name: e.name().to_string(),
                entity_type: e.entity_type().to_string(),
                resolution: r.strategy.clone(),
            }
        })
        .collect();

    // One question, one box. Runs of same-scope boxes separated by
    // a short agreed gap become one, with the gap duplicated into both claims —
    // so the two standard resolutions are byte-identical to what they were, and
    // only the number of decisions the reader is asked for changes.
    let content = crate::conflict::coalesce_adjacent_boxes(&content, marker_format);
    let content = restore.apply(content);

    // The runtime self-check. Every check in the suite is about the
    // decisions; this one is about the finished bytes of a CONFLICTED file, in
    // the region the decisions do not cover — the frame, outside every box. The
    // suite rules it out over a finite set of cases; this reports it on the
    // file in front of the user, where the set of cases is not finite.
    //
    // Asked only of conflicted outputs, and advisory even then: a clean merge's
    // bytes are already under `drops_unanimous_lines` and the fold's own
    // backstops, and a conflicted file already needs a human — so this can point
    // and must never re-decide.
    let mut warnings = bind_report.findings;
    if crate::frame::is_conflicted(&content) {
        warnings.extend(
            crate::frame::frame_duplicates(base, ours, theirs, &content)
                .into_iter()
                .map(|d| SemanticWarning {
                    entity_name: d.line.trim().to_string(),
                    entity_type: "frame".to_string(),
                    file_path: file_path.to_string(),
                    kind: WarningKind::ConflictFrameDuplicate {
                        line: d.line,
                        found: d.found,
                        allowed: d.allowed,
                    },
                    related: Vec::new(),
                }),
        );
    }

    Ok(MergeResult {
        content,
        conflicts,
        warnings,
        stats,
        audit,
    })
}

/// Fold the finished dispositions into the merge summary.
///
/// Every counter here is a *derived read*: `MergeStats` states nothing that the
/// dispositions do not already say, so it cannot contradict them. That is why
/// no stage takes `&mut MergeStats` any more — `resolve` returns its tally with
/// its decision, `bind` replaces the tally when it revises a decision, and the
/// arithmetic happens exactly once, at the end, in the stage that owns the
/// result.
///
/// `resolved_via_*` are read off the strategy rather than tallied separately,
/// for the same reason: which ladder rung merged a body is already recorded in
/// the strategy, and a second recording of it is a second thing to keep true.
fn summarize(resolved: &[resolve::Resolved]) -> MergeStats {
    let mut s = MergeStats::default();
    for r in resolved {
        // The buckets belong to `MergeStats`, not to this loop: see
        // `MergeStats::record`. This walks the decisions; it does not own the
        // summary's fields.
        s.record(&r.tally, &r.strategy);
    }
    s
}

/// The line-ending and BOM shape all three inputs agreed on, if they agreed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Encoding {
    crlf: bool,
    bom: bool,
}

impl Encoding {
    fn of(base: &str, ours: &str, theirs: &str) -> Self {
        let all = |f: fn(&str) -> bool| f(base) && f(ours) && f(theirs);
        Encoding {
            crlf: all(|s| s.contains("\r\n")),
            bom: all(|s| s.starts_with('\u{feff}')),
        }
    }
    fn apply(self, content: String) -> String {
        let content = if self.crlf {
            content.replace('\n', "\r\n")
        } else {
            content
        };
        if self.bom {
            format!("\u{feff}{content}")
        } else {
            content
        }
    }
}

/// Pair each parsed entity with its *region* content — the file's own bytes,
/// including leading doc comments and modifiers — falling back to the parser's
/// narrower slice when a region is missing.
///
/// The one place that pairing is made. Both doors into the pipeline
/// ([`read_version`], which stops after classification, and [`merge_file`],
/// the real merge) go through it, so neither can accidentally feed the matcher a
/// different notion of "this entity's text" than the other and make their
/// verdicts incomparable.
fn raws(
    entities: &[sem_core::model::entity::SemanticEntity],
    regions: &[FileRegion],
    separator: Option<&str>,
) -> Vec<RawEntity> {
    let mut by_id: HashMap<&str, &str> = HashMap::new();
    for r in regions {
        if let FileRegion::Entity(e) = r {
            by_id.insert(e.entity_id.as_str(), e.content.as_str());
        }
    }
    entities
        .iter()
        .map(|e| RawEntity {
            name: e.name.clone(),
            entity_type: e.entity_type.clone(),
            parent: e.parent_id.clone(),
            content: strip_separator(
                by_id
                    .get(e.id.as_str())
                    .copied()
                    .unwrap_or(e.content.as_str()),
                separator,
            ),
            src_id: e.id.clone(),
        })
        .collect()
}

/// Drop the trailing separator: it states this entity's POSITION in the
/// sequence, and nothing about the entity. See [`entity_separator`].
///
/// The whitespace behind it is left exactly as it was — the separator is what
/// the encoding contributes, the newline is the file's own layout.
fn strip_separator(text: &str, separator: Option<&str>) -> String {
    let Some(sep) = separator else {
        return text.to_string();
    };
    let trimmed = text.trim_end();
    match trimmed.strip_suffix(sep) {
        Some(body) => format!("{body}{}", &text[trimmed.len()..]),
        None => text.to_string(),
    }
}
