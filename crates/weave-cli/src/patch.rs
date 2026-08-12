//! `weave patch` — the WRITE side of the agent contract.
//!
//! The read side (`weave-findings.schema.json`) tells an agent what a merge
//! *did*. This is the symmetric half: how an agent says what it *wants*, in the
//! same vocabulary, over the same keys.
//!
//! # Why ops rather than a diff
//!
//! A textual diff is a statement about line numbers, and line numbers are the
//! first thing to rot. `extract` emits a typed op per entity, drawn from a
//! fixed set: `added`, `edited`, `renamed`, `rename_edited`,
//! `deleted`. An op names an *entity*, so
//! it survives everything that moved around it.
//!
//! # apply is a merge, not a patch
//!
//! The interesting case is a target that has **drifted** from the base the ops
//! were extracted against. `git apply` answers that with fuzz and rejects;
//! weave answers it with the merge it already knows how to do:
//!
//! ```text
//!   apply(ops, target) = entity_merge(
//!       base   = the base the ops were extracted from,
//!       ours   = target,                      // what is actually on disk
//!       theirs = naive_apply(ops, base),      // what the ops intended
//!   )
//! ```
//!
//! This formulation is what makes drift honest. `theirs` is the author's intent
//! reconstructed *in the author's own world*; `ours` is reality; `base` is the
//! common ancestor they actually share. Anything else — applying ops straight to
//! a drifted target — silently resolves a three-way situation with two-way
//! information, and that is exactly where patch tools lose edits.
//!
//! When the ops carry no base, or the target *is* the base, the merge degenerates
//! (`ours == base` ⟹ result is `theirs`) and `apply` short-circuits to the naive
//! application. That degeneracy falls out of 3-way merge itself, not a special
//! case bolted on — and it keeps the naive result byte-for-byte.
//!
//! # The exactness escape hatch
//!
//! `extract` **verifies itself**: it applies the ops it just produced back onto
//! the base and compares with the changed file. When entity granularity cannot
//! express the change (a rewritten module docstring, reordered entities, a
//! changed shebang), it says so — `exact: false` — and appends a whole-file
//! fallback op. The read side has no counterpart — a line-level fallback
//! merge returns an empty audit trail there, so it reports nothing. The
//! round trip then still holds byte-for-byte, and the document is honest about
//! having lost entity granularity instead of quietly dropping the difference.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use sem_core::model::change::ChangeType;
use sem_core::model::identity::match_entities;
use weave_core::binding::replace_at_word_boundaries;
use weave_core::conflict::EntityConflict;
use weave_core::MergeResult;

use crate::parsers::entities_of;
use crate::wire::{conflict_advice, Finding, Sides};

/// Semver of the ops wire format (`schema/weave-ops.schema.json`).
pub(crate) const OPS_SCHEMA_VERSION: &str = "1.0.0";

/// The entity key used for a whole-file fallback op. Mirrors the read side's
/// convention for findings that are about the file rather than an entity.
pub const FILE_KEY: &str = "(file)";

/// Why an ops document could not be applied.
///
/// One refusal today, and it is the one worth naming: the caller supplied a
/// base that is provably not the one the ops were extracted from. It used to
/// be a `String` carrying both hashes inside a sentence, so the only way to
/// tell this refusal from any other was to read the prose. Both operands now
/// travel as fields, and a caller can decide (re-extract? drop the base? stop?)
/// by matching.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PatchError {
    /// The supplied base does not hash to what the ops were extracted against.
    #[error(
        "base mismatch: the ops were extracted against sha256 {expected}, \
         but the supplied base hashes to {actual}"
    )]
    BaseMismatch {
        /// The digest the document declares.
        expected: String,
        /// The digest the supplied base actually has.
        actual: String,
    },
}

// ---------------------------------------------------------------------------
// Wire format
// ---------------------------------------------------------------------------

/// The op alphabet, exactly as `schema/weave-ops.schema.json` enumerates it.
///
/// It used to be a `String`, so a document saying `"op": "eddited"` decoded
/// without complaint and then fell through this crate's `match` into the
/// "edited-ish" arm — an unknown verb silently treated as a known one. Being
/// an enum makes the whole vocabulary a parsing question, answered once, at the
/// boundary, before anything acts on it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    /// The entity is new in the changed file.
    Added,
    /// The entity's text changed.
    Edited,
    /// The entity was renamed, its text otherwise unchanged.
    Renamed,
    /// Renamed and edited in one step.
    RenameEdited,
    /// The entity is gone from the changed file.
    Deleted,
    /// Carries no intent; a v1 producer never emits it.
    Unchanged,
    /// Reserved: relational, correlating several entity keys, which a per-key
    /// matcher cannot express. A v1 consumer accepts them and acts on none —
    /// which is a thing this type can now state rather than a thing the
    /// `match` below happens to do.
    Split,
    /// Reserved. See [`Op::Split`].
    Merged,
    /// Reserved. See [`Op::Split`].
    Copied,
    /// Reserved. See [`Op::Split`].
    Moved,
}

impl Op {
    /// The verb as the wire format spells it — the one place the mapping
    /// lives, so a diagnostic and the JSON cannot drift apart.
    pub fn wire(self) -> &'static str {
        match self {
            Op::Added => "added",
            Op::Edited => "edited",
            Op::Renamed => "renamed",
            Op::RenameEdited => "rename_edited",
            Op::Deleted => "deleted",
            Op::Unchanged => "unchanged",
            Op::Split => "split",
            Op::Merged => "merged",
            Op::Copied => "copied",
            Op::Moved => "moved",
        }
    }
}

/// Why an ops document could not be read.
///
/// `weave patch apply` used to answer both of these with a `format!` string,
/// so "this is not JSON", "this is JSON but not an ops document" and "this is
/// an ops document from a future major" were one type. The first two are a
/// caller's typo; the third means "upgrade weave", and a caller cannot act on
/// that difference by reading a sentence.
#[derive(Debug, thiserror::Error)]
pub enum OpsDocError {
    /// The bytes did not decode into an ops document: not JSON, a field of the
    /// wrong type, a required field missing, an op outside the vocabulary, or
    /// a field the schema does not define.
    #[error("not a weave ops document: {source}")]
    Malformed {
        #[source]
        source: serde_json::Error,
    },

    /// A major version this build does not implement.
    #[error("schema_version {found} is a major this build does not know (it implements 1.x)")]
    UnknownMajor { found: String },
}

/// Decode an ops document, or say which way it was not one.
///
/// The one entry point: nothing else in this crate turns bytes into an
/// [`OpsDoc`], and no `serde_json::Value` survives past this line.
pub fn parse_ops_doc(text: &str) -> Result<OpsDoc, OpsDocError> {
    let doc: OpsDoc =
        serde_json::from_str(text).map_err(|source| OpsDocError::Malformed { source })?;
    if !doc.schema_version.starts_with("1.") {
        return Err(OpsDocError::UnknownMajor {
            found: doc.schema_version,
        });
    }
    Ok(doc)
}

/// `deny_unknown_fields` here and on the two types below is not strictness for
/// its own sake: `schema/weave-ops.schema.json` says `additionalProperties:
/// false` at all three levels, and until now this decoder accepted documents
/// its own published schema rejects. A misspelled `entitiy` used to be
/// discarded in silence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OpsDoc {
    pub schema_version: String,
    /// The path the ops were extracted from. Its extension selects the parser;
    /// `apply` may target a different path entirely.
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<BaseRef>,
    /// True when applying these ops to the declared base reproduces the changed
    /// file byte-for-byte at entity granularity. False means a whole-file
    /// fallback op is present and granularity was lost.
    pub exact: bool,
    pub ops: Vec<PatchOp>,
}

/// What the ops were extracted against.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BaseRef {
    /// SHA-256 of the base content. Lets `apply` refuse a base that is not the
    /// one the ops were computed from, instead of merging against a lie.
    pub sha256: String,
    /// The base itself. Optional and usually large; when present the ops are
    /// self-contained and `apply` can do the real three-way merge with no help.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PatchOp {
    /// Which of the op vocabulary's actions this is.
    pub op: Op,
    /// The key the op is about: the entity's name **in the base**, so a rename
    /// can still be located in a target that has not seen it yet.
    pub entity: String,
    pub entity_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// The entity's full new text. Absent for `deleted`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// For `added`: the entity this one follows in the changed file. Absent
    /// means "at the top of the file". A missing anchor in the target is not an
    /// error — the entity is appended, and the op is reported as placed by
    /// fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// For `added`: the whitespace/comment gap that separated this entity from
    /// its anchor, so an insertion does not weld two definitions together.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub separator: Option<String>,
    /// True on the whole-file escape hatch (`entity == "(file)"`): weave could
    /// not express the change at entity granularity and is shipping the file.
    #[serde(default, skip_serializing_if = "is_false")]
    pub fallback: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

pub(crate) fn sha256_hex(content: &str) -> String {
    let mut h = Sha256::new();
    h.update(content.as_bytes());
    format!("{:x}", h.finalize())
}

// ---------------------------------------------------------------------------
// Regions: an entity plus the comments that belong to it
// ---------------------------------------------------------------------------

/// A top-level entity's line span, 0-based, `[start, end)`.
#[derive(Debug, Clone)]
struct Region {
    name: String,
    start: usize,
    end: usize,
}

/// Is this line a comment or decorator that belongs to the definition below it?
fn is_lead_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//")
        || t.starts_with('#')
        || t.starts_with("/*")
        || t.starts_with('*')
        || t.starts_with('@')
        || t.starts_with("--")
        || t.starts_with(";;")
}

/// Split `content` into top-level entity regions, bundling each entity's
/// leading comment/decorator block into it.
///
/// Bundling matches what weave-core's reconstruction does, and it is not
/// cosmetic: without it, editing a function's docstring is not expressible as an
/// op on that function, and every docstring edit would fall back to whole-file.
fn regions_of(path: &str, content: &str) -> Vec<Region> {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let mut regions: Vec<Region> = Vec::new();
    let mut floor = 0usize; // no region may start before this
    for e in entities_of(path, content) {
        let start0 = e.start_line.saturating_sub(1).min(total);
        let end0 = e.end_line.min(total);
        if end0 <= start0 {
            continue;
        }
        let mut bundled = start0;
        while bundled > floor && is_lead_line(lines[bundled - 1]) {
            bundled -= 1;
        }
        regions.push(Region {
            name: e.name.clone(),
            start: bundled,
            end: end0,
        });
        floor = end0;
    }
    regions
}

fn slice(lines: &[&str], start: usize, end: usize) -> String {
    if start >= end {
        return String::new();
    }
    let mut s = lines[start..end].join("\n");
    s.push('\n');
    s
}

// ---------------------------------------------------------------------------
// extract
// ---------------------------------------------------------------------------

/// Compute the ops that turn `base` into `changed`.
///
/// `embed_base` inlines the base snapshot, making the document self-contained —
/// which is what lets `apply` run a real three-way merge against a drifted
/// target with nothing but the ops file.
pub fn extract(path: &str, base: &str, changed: &str, embed_base: bool) -> OpsDoc {
    let base_ents = entities_of(path, base);
    let changed_ents = entities_of(path, changed);

    let changed_lines: Vec<&str> = changed.lines().collect();
    let changed_regions = regions_of(path, changed);
    let region_text = |name: &str| -> Option<String> {
        changed_regions
            .iter()
            .find(|r| r.name == name)
            .map(|r| slice(&changed_lines, r.start, r.end))
    };

    let mut ops: Vec<PatchOp> = Vec::new();
    let changes = match_entities(&base_ents, &changed_ents, path, None, None, None).changes;

    // Ops are keyed by BASE names, because that is what `apply` can still find
    // in a target that has not seen the patch. An anchor named after a renamed
    // entity has to be translated back, or the insertion loses its place.
    let to_base_name: std::collections::HashMap<String, String> = changes
        .iter()
        .filter(|c| c.change_type == ChangeType::Renamed)
        .filter_map(|c| {
            c.old_entity_name
                .clone()
                .map(|old| (c.entity_name.clone(), old))
        })
        .collect();

    for c in &changes {
        let name = c.entity_name.clone();
        match c.change_type {
            ChangeType::Added => {
                let (after, separator) = anchor_for(&changed_regions, &changed_lines, &name);
                let after = after.map(|a| to_base_name.get(&a).cloned().unwrap_or(a));
                ops.push(PatchOp {
                    op: Op::Added,
                    entity: name.clone(),
                    entity_type: c.entity_type.clone(),
                    from: None,
                    to: None,
                    content: region_text(&name),
                    after,
                    separator,
                    fallback: false,
                });
            }
            ChangeType::Deleted => ops.push(PatchOp {
                op: Op::Deleted,
                entity: name.clone(),
                entity_type: c.entity_type.clone(),
                from: None,
                to: None,
                content: None,
                after: None,
                separator: None,
                fallback: false,
            }),
            ChangeType::Renamed => {
                let old = c.old_entity_name.clone().unwrap_or_else(|| name.clone());
                let new_text = region_text(&name);
                // Pure rename vs rename+edit: substitute the old name back and
                // see whether the base body returns — decide on text, not a guess.
                let base_text = base_ents
                    .iter()
                    .find(|e| e.name == old)
                    .map(|_| region_slice(path, base, &old))
                    .unwrap_or_default();
                let unrenamed = new_text
                    .as_deref()
                    .map(|t| replace_at_word_boundaries(t, &name, &old))
                    .unwrap_or_default();
                let op = if !base_text.is_empty() && unrenamed == base_text {
                    Op::Renamed
                } else {
                    Op::RenameEdited
                };
                ops.push(PatchOp {
                    op,
                    entity: old.clone(),
                    entity_type: c.entity_type.clone(),
                    from: Some(old),
                    to: Some(name.clone()),
                    content: new_text,
                    after: None,
                    separator: None,
                    fallback: false,
                });
            }
            // Modified, and the positional changes weave cannot express as
            // anything finer. `edited` is the honest description; if position
            // actually mattered, self-verification below will catch it.
            ChangeType::Modified | ChangeType::Moved | ChangeType::Reordered => ops.push(PatchOp {
                op: Op::Edited,
                entity: name.clone(),
                entity_type: c.entity_type.clone(),
                from: None,
                to: None,
                content: region_text(&name),
                after: None,
                separator: None,
                fallback: false,
            }),
        }
    }

    // The matcher compares entity *bodies*, so a change confined to an entity's
    // leading comment block registers as no change at all. The region does know
    // — and a docstring edit that silently vanished from a patch would be worse
    // than one that fell back to whole-file. Sweep for it explicitly.
    let touched: std::collections::HashSet<&str> = changes
        .iter()
        .map(|c| c.entity_name.as_str())
        .chain(changes.iter().filter_map(|c| c.old_entity_name.as_deref()))
        .collect();
    let base_regions = regions_of(path, base);
    let base_lines: Vec<&str> = base.lines().collect();
    for r in &base_regions {
        if touched.contains(r.name.as_str()) {
            continue;
        }
        let Some(new_text) = region_text(&r.name) else {
            continue;
        };
        let old_text = slice(&base_lines, r.start, r.end);
        if old_text != new_text {
            let entity_type = base_ents
                .iter()
                .find(|e| e.name == r.name)
                .map(|e| e.entity_type.clone())
                .unwrap_or_else(|| "unknown".to_string());
            ops.push(PatchOp {
                op: Op::Edited,
                entity: r.name.clone(),
                entity_type,
                from: None,
                to: None,
                content: Some(new_text),
                after: None,
                separator: None,
                fallback: false,
            });
        }
    }

    // Deletions first, then edits, then additions: additions anchor on entities
    // that must still be findable when they are placed.
    ops.sort_by_key(|o| match o.op {
        Op::Deleted => 0,
        Op::Added => 2,
        _ => 1,
    });

    let mut doc = OpsDoc {
        schema_version: OPS_SCHEMA_VERSION.to_string(),
        file: path.to_string(),
        base: Some(BaseRef {
            sha256: sha256_hex(base),
            inline: embed_base.then(|| base.to_string()),
        }),
        exact: true,
        ops,
    };

    // ---- Self-verification. The document does not get to claim what it has
    // not demonstrated.
    let replay = apply_ops(base, path, &doc);
    if replay.content != changed {
        doc.exact = false;
        doc.ops.push(PatchOp {
            op: Op::Edited,
            entity: FILE_KEY.into(),
            entity_type: "file".into(),
            from: None,
            to: None,
            content: Some(changed.to_string()),
            after: None,
            separator: None,
            fallback: true,
        });
    }
    doc
}

fn region_slice(path: &str, content: &str, name: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    regions_of(path, content)
        .iter()
        .find(|r| r.name == name)
        .map(|r| slice(&lines, r.start, r.end))
        .unwrap_or_default()
}

/// Where a newly added entity sits, expressed relative to its neighbours rather
/// than to a line number.
fn anchor_for(regions: &[Region], lines: &[&str], name: &str) -> (Option<String>, Option<String>) {
    let Some(idx) = regions.iter().position(|r| r.name == name) else {
        return (None, None);
    };
    if idx > 0 {
        let prev = &regions[idx - 1];
        let gap = slice(lines, prev.end, regions[idx].start);
        (Some(prev.name.clone()), Some(gap))
    } else {
        // First in the file: the gap that follows it is what keeps it apart
        // from whatever came first before.
        let gap = regions
            .get(idx + 1)
            .map(|next| slice(lines, regions[idx].end, next.start))
            .unwrap_or_default();
        (None, Some(gap))
    }
}

// ---------------------------------------------------------------------------
// naive application — "what the ops intended, in the author's own world"
// ---------------------------------------------------------------------------

/// A pending line-range rewrite, expressed over the ORIGINAL line indices so
/// that ops stay order-independent.
struct Edit {
    /// Position in the ops document; only used to break ties between two
    /// insertions at the same line.
    order: usize,
    start: usize,
    end: usize,
    replacement: Vec<String>,
}

impl Edit {
    fn is_insertion(&self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NaiveApply {
    pub content: String,
    /// Ops whose target entity was not present. Never silent: an op that did
    /// not land is reported, because "applied nothing" and "applied everything"
    /// must not look the same.
    pub unapplied: Vec<String>,
}

/// Apply ops to `source` positionally, with no merge. This is the `theirs`
/// synthesis step of `apply`, and on its own it is only correct when `source`
/// is the base the ops were extracted from.
pub(crate) fn apply_ops(source: &str, path: &str, doc: &OpsDoc) -> NaiveApply {
    // The whole-file escape hatch wins outright.
    if let Some(op) = doc
        .ops
        .iter()
        .rev()
        .find(|o| o.fallback && o.entity == FILE_KEY)
    {
        return NaiveApply {
            content: op.content.clone().unwrap_or_default(),
            unapplied: Vec::new(),
        };
    }

    let lines: Vec<&str> = source.lines().collect();
    let trailing_newline = source.is_empty() || source.ends_with('\n');
    let regions = regions_of(path, source);
    let find = |name: &str| regions.iter().position(|r| r.name == name);

    let mut edits: Vec<Edit> = Vec::new();
    let mut unapplied: Vec<String> = Vec::new();
    let to_lines = |s: &str| -> Vec<String> { s.lines().map(|l| l.to_string()).collect() };
    let mut push = |start: usize, end: usize, replacement: Vec<String>| {
        edits.push(Edit {
            order: edits.len(),
            start,
            end,
            replacement,
        })
    };

    for op in &doc.ops {
        match op.op {
            Op::Deleted => match find(&op.entity) {
                Some(i) => {
                    let r = &regions[i];
                    // Take the entity *and* the gap that follows it, so removing
                    // a definition does not leave its blank lines behind. When
                    // it is last, take the gap before it instead.
                    let (start, end) = match regions.get(i + 1) {
                        Some(next) => (r.start, next.start),
                        None if i > 0 => (regions[i - 1].end, r.end),
                        None => (r.start, r.end),
                    };
                    push(start, end, Vec::new());
                }
                None => unapplied.push(format!("deleted {}", op.entity)),
            },
            Op::Edited | Op::Renamed | Op::RenameEdited => match find(&op.entity) {
                Some(i) => {
                    let r = &regions[i];
                    push(
                        r.start,
                        r.end,
                        to_lines(op.content.as_deref().unwrap_or("")),
                    );
                }
                None => unapplied.push(format!("{} {}", op.op.wire(), op.entity)),
            },
            Op::Added => {
                let text = op.content.clone().unwrap_or_default();
                let sep = op.separator.clone().unwrap_or_default();
                let (at, payload) = match &op.after {
                    Some(anchor) => match find(anchor) {
                        Some(i) => (regions[i].end, format!("{sep}{text}")),
                        // Anchor gone: append rather than drop. Reported, so a
                        // caller knows placement was a fallback.
                        None => {
                            unapplied
                                .push(format!("added {} (anchor `{anchor}` absent)", op.entity));
                            (lines.len(), format!("\n{text}"))
                        }
                    },
                    None => (0, format!("{text}{sep}")),
                };
                push(at, at, to_lines(&payload));
            }
            // `unchanged` and the reserved relational ops: accepted, acted on
            // by nobody. Now spelled as the variants they are, so adding a
            // verb to the vocabulary makes this arm a compile error rather
            // than a silent no-op.
            Op::Unchanged | Op::Split | Op::Merged | Op::Copied | Op::Moved => {}
        }
    }

    // Apply back-to-front so earlier line indices stay valid. Two tie-breaks
    // matter at a shared position: a removal/replacement must go before an
    // insertion anchored at the same line (otherwise the insertion lands inside
    // the range about to be removed), and insertions among themselves are
    // applied in reverse document order so they come out in document order.
    edits.sort_by(|a, b| {
        b.start
            .cmp(&a.start)
            .then(a.is_insertion().cmp(&b.is_insertion()))
            .then(b.order.cmp(&a.order))
    });

    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    for e in edits {
        let start = e.start.min(out.len());
        let end = e.end.clamp(start, out.len());
        out.splice(start..end, e.replacement);
    }

    let mut content = out.join("\n");
    if trailing_newline && !content.is_empty() {
        content.push('\n');
    }
    NaiveApply { content, unapplied }
}

// ---------------------------------------------------------------------------
// apply — the three-way formulation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyMode {
    /// `ours == base`: the three-way merge degenerates to `theirs`.
    FastForward,
    /// The target drifted from the base; a real entity merge decided it.
    ThreeWay,
    /// No base was available, so the target was treated as one. Drift cannot be
    /// detected in this mode — it is stated, not hidden.
    NoBase,
}

impl ApplyMode {
    pub fn wire(self) -> &'static str {
        match self {
            ApplyMode::FastForward => "fast_forward",
            ApplyMode::ThreeWay => "three_way",
            ApplyMode::NoBase => "no_base",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApplyReport {
    pub content: String,
    pub clean: bool,
    pub mode: ApplyMode,
    pub findings: Vec<Finding>,
    pub unapplied: Vec<String>,
}

/// Apply `doc` to `target`.
///
/// `declared_base` is the base the caller supplied out of band; the document's
/// own `base.inline` is used when it has one. With neither, the target stands in
/// for the base and the result is a naive application (`NoBase`).
///
/// Errors only when the caller supplied a base that is provably not the one the
/// ops were computed from — merging against the wrong ancestor produces a
/// confidently wrong answer, which is the one outcome worth refusing.
pub fn apply(
    doc: &OpsDoc,
    target: &str,
    target_path: &str,
    declared_base: Option<&str>,
    host: &weave_core::host::Host,
) -> Result<ApplyReport, PatchError> {
    let base: Option<String> = declared_base
        .map(|s| s.to_string())
        .or_else(|| doc.base.as_ref().and_then(|b| b.inline.clone()));

    if let (Some(b), Some(r)) = (base.as_deref(), doc.base.as_ref()) {
        let actual = sha256_hex(b);
        if actual != r.sha256 {
            return Err(PatchError::BaseMismatch {
                expected: r.sha256.clone(),
                actual,
            });
        }
    }

    let Some(base) = base else {
        let naive = apply_ops(target, target_path, doc);
        return Ok(ApplyReport {
            content: naive.content,
            clean: true,
            mode: ApplyMode::NoBase,
            findings: Vec::new(),
            unapplied: naive.unapplied,
        });
    };

    let theirs = apply_ops(&base, target_path, doc);

    if base == target {
        // ours == base ⟹ the three-way merge is theirs, by definition. Taking
        // the shortcut keeps the round trip byte-exact instead of subjecting an
        // unambiguous result to reconstruction.
        return Ok(ApplyReport {
            content: theirs.content,
            clean: true,
            mode: ApplyMode::FastForward,
            findings: Vec::new(),
            unapplied: theirs.unapplied,
        });
    }

    let result = weave_core::entity_merge_fmt(
        &base,
        target,
        &theirs.content,
        target_path,
        &weave_core::MarkerFormat::default(),
        host,
    );
    let findings = findings_of(&result);
    Ok(ApplyReport {
        clean: result.is_clean(),
        content: result.content,
        mode: ApplyMode::ThreeWay,
        findings,
        unapplied: theirs.unapplied,
    })
}

/// Conflicts of a merge, in the read side's finding vocabulary.
///
/// The kind-and-advice mapping is `crate::wire::conflict_advice`, shared with
/// `weave-mcp`'s findings document; the only thing this producer chooses is
/// what to call the two sides.
pub(crate) fn findings_of(result: &MergeResult) -> Vec<Finding> {
    result.conflicts.iter().map(finding_of_conflict).collect()
}

fn finding_of_conflict(c: &EntityConflict) -> Finding {
    let (kind, suggestion) = conflict_advice(c, Sides::PATCH);

    let mut f = Finding::derived("CONFLICT", &c.entity_name, &c.entity_type);
    f.source = "conflict".to_string();
    f.kind = Some(kind.to_string());
    f.base = c.base_content.clone();
    f.ours = c.ours_content.clone();
    f.theirs = c.theirs_content.clone();
    f.complexity = Some(c.complexity.to_string());
    f.suggestion = Some(suggestion);
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "\
def alpha(x):
    return x + 1


def beta(x):
    return x * 2


def gamma(x):
    return x - 3
";

    fn roundtrip(base: &str, changed: &str) -> OpsDoc {
        let doc = extract("m.py", base, changed, true);
        let out =
            apply(&doc, base, "m.py", None, &weave_core::host::Host::default()).expect("apply");
        assert_eq!(out.content, changed, "round trip must be byte-exact");
        doc
    }

    // ---- the round-trip property ----------------------------------------

    #[test]
    fn extract_then_apply_reproduces_an_edit_exactly_at_entity_granularity() {
        let changed = BASE.replace("return x * 2", "return x * 22");
        let doc = roundtrip(BASE, &changed);
        assert!(doc.exact, "an in-body edit is expressible as one op");
        assert_eq!(doc.ops.len(), 1);
        assert_eq!(doc.ops[0].op, Op::Edited);
        assert_eq!(doc.ops[0].entity, "beta");
    }

    #[test]
    fn extract_then_apply_reproduces_a_rename_exactly() {
        let changed = BASE.replace("def beta(x):", "def bravo(x):");
        let doc = roundtrip(BASE, &changed);
        assert!(doc.exact);
        assert_eq!(doc.ops[0].op, Op::Renamed);
        assert_eq!(doc.ops[0].from.as_deref(), Some("beta"));
        assert_eq!(doc.ops[0].to.as_deref(), Some("bravo"));
    }

    /// **A known matcher gap, pinned deliberately.**
    ///
    /// Findings are only as good as the matcher they're built on: a rename that
    /// also rewrites the body should still pair across the rename, but
    /// sem-core's matcher does not pair it here, so `extract` sees a delete and
    /// an add, and that is what it emits.
    ///
    /// It is emitted honestly rather than papered over with a local
    /// similarity heuristic: `apply` synthesises `theirs` and hands it to
    /// `entity_merge`, which runs the *same* matcher again — so a locally
    /// invented pairing would be undone downstream while making this document
    /// disagree with weave-core about what happened. The round trip is still
    /// exact; the cost is fidelity against a drifted target, where a
    /// rename+edit degrades to modify/delete instead of rename/modify.
    #[test]
    fn a_rename_that_also_edits_the_body_degrades_to_delete_plus_add_under_the_current_matcher() {
        let changed = BASE
            .replace("def beta(x):", "def bravo(x):")
            .replace("return x * 2", "return x * 22");
        let doc = roundtrip(BASE, &changed);
        let mut kinds: Vec<&str> = doc.ops.iter().map(|o| o.op.wire()).collect();
        kinds.sort_unstable();
        assert_eq!(kinds, vec!["added", "deleted"]);
        assert!(doc.exact, "still byte-exact, just coarser than it could be");
    }

    /// `rename_edited` stays in the vocabulary and stays *applicable*: a
    /// hand-authored op (or a future matcher that pairs it) must land.
    #[test]
    fn a_hand_authored_rename_edited_op_applies() {
        let doc = OpsDoc {
            schema_version: OPS_SCHEMA_VERSION.into(),
            file: "m.py".into(),
            base: None,
            exact: true,
            ops: vec![PatchOp {
                op: Op::RenameEdited,
                entity: "beta".into(),
                entity_type: "function".into(),
                from: Some("beta".into()),
                to: Some("bravo".into()),
                content: Some("def bravo(x):\n    return x * 22\n".into()),
                after: None,
                separator: None,
                fallback: false,
            }],
        };
        let out = apply_ops(BASE, "m.py", &doc);
        assert!(out.unapplied.is_empty());
        assert!(out.content.contains("def bravo(x):\n    return x * 22"));
        assert!(!out.content.contains("def beta"));
        assert!(out.content.contains("def alpha") && out.content.contains("def gamma"));
    }

    #[test]
    fn extract_then_apply_reproduces_a_deletion_exactly() {
        let changed = "\
def alpha(x):
    return x + 1


def gamma(x):
    return x - 3
";
        let doc = roundtrip(BASE, changed);
        assert!(doc.exact, "deleting an entity takes its gap with it");
        assert_eq!(doc.ops[0].op, Op::Deleted);
        assert_eq!(doc.ops[0].entity, "beta");
    }

    #[test]
    fn extract_then_apply_reproduces_an_addition_exactly() {
        let changed = format!("{BASE}\n\ndef delta(x):\n    return x / 4\n");
        let doc = roundtrip(BASE, &changed);
        assert!(doc.exact);
        let added = doc
            .ops
            .iter()
            .find(|o| o.op == Op::Added)
            .expect("added op");
        assert_eq!(added.entity, "delta");
        assert_eq!(added.after.as_deref(), Some("gamma"));
    }

    #[test]
    fn extract_then_apply_reproduces_all_four_op_kinds_at_once() {
        let changed = "\
def alpha(x):
    return x + 100


def bravo(x):
    return x * 2


def delta(x):
    return x / 4
";
        let doc = roundtrip(BASE, changed);
        let mut kinds: Vec<&str> = doc.ops.iter().map(|o| o.op.wire()).collect();
        kinds.sort_unstable();
        assert_eq!(kinds, vec!["added", "deleted", "edited", "renamed"]);
        assert!(
            doc.exact,
            "the whole change is expressible as ops: {doc:#?}"
        );
    }

    #[test]
    fn a_docstring_edit_is_an_op_on_the_entity_it_documents() {
        let base = "# lead comment\ndef alpha(x):\n    return x\n";
        let changed = "# better comment\ndef alpha(x):\n    return x\n";
        let doc = roundtrip(base, changed);
        assert!(doc.exact, "leading comments bundle into the entity");
        assert_eq!(doc.ops[0].entity, "alpha");
    }

    // ---- the escape hatch ------------------------------------------------

    #[test]
    fn a_change_outside_every_entity_is_reported_as_inexact_and_still_round_trips() {
        // The trailing module-level statement belongs to no entity.
        let changed = format!("{BASE}\nprint(alpha(1))\n");
        let doc = roundtrip(BASE, &changed);
        assert!(
            !doc.exact,
            "weave must not claim entity granularity it lacks"
        );
        let last = doc.ops.last().unwrap();
        assert!(last.fallback);
        assert_eq!(last.entity, FILE_KEY);
    }

    // ---- drift ------------------------------------------------------------

    #[test]
    fn ops_land_cleanly_on_a_target_with_a_disjoint_edit() {
        // The patch edits beta …
        let changed = BASE.replace("return x * 2", "return x * 22");
        let doc = extract("m.py", BASE, &changed, true);
        // … while the target has independently edited gamma.
        let target = BASE.replace("return x - 3", "return x - 33");

        let out = apply(
            &doc,
            &target,
            "m.py",
            None,
            &weave_core::host::Host::default(),
        )
        .expect("apply");
        assert_eq!(out.mode, ApplyMode::ThreeWay, "the target drifted");
        assert!(out.clean, "disjoint entities compose");
        assert!(
            out.content.contains("return x * 22"),
            "the patch's edit landed"
        );
        assert!(
            out.content.contains("return x - 33"),
            "the target's edit survived"
        );
        assert!(out.findings.is_empty());
    }

    #[test]
    fn a_deletion_lands_on_a_drifted_target() {
        let changed = BASE.replace("def beta(x):\n    return x * 2\n\n\n", "");
        let doc = extract("m.py", BASE, &changed, true);
        let target = BASE.replace("return x + 1", "return x + 11");
        let out = apply(
            &doc,
            &target,
            "m.py",
            None,
            &weave_core::host::Host::default(),
        )
        .expect("apply");
        assert!(out.clean, "{:#?}", out.findings);
        assert!(!out.content.contains("def beta"));
        assert!(out.content.contains("return x + 11"));
    }

    // ---- honest conflict --------------------------------------------------

    #[test]
    fn divergent_renames_conflict_rather_than_pick_a_winner() {
        let changed = BASE.replace("def beta(x):", "def bravo(x):");
        let doc = extract("m.py", BASE, &changed, true);
        // The target renamed the same entity to something else.
        let target = BASE.replace("def beta(x):", "def charlie(x):");

        let out = apply(
            &doc,
            &target,
            "m.py",
            None,
            &weave_core::host::Host::default(),
        )
        .expect("apply");
        assert!(!out.clean, "weave must refuse to guess between two names");
        assert_eq!(out.mode, ApplyMode::ThreeWay);
        let f = out.findings.first().expect("a CONFLICT finding");
        assert_eq!(f.class, "CONFLICT");
        assert_eq!(f.source, "conflict");
        assert_eq!(f.kind.as_deref(), Some("rename_rename"));
        assert!(f.suggestion.as_ref().unwrap().contains("call site"));
        assert!(
            out.content.contains("<<<<<<<") || out.content.contains("======="),
            "a conflict renders as markers, not as a silent choice:\n{}",
            out.content
        );
    }

    #[test]
    fn both_sides_editing_one_body_conflicts() {
        let changed = BASE.replace("return x * 2", "return x * 22");
        let doc = extract("m.py", BASE, &changed, true);
        let target = BASE.replace("return x * 2", "return x * 222");
        let out = apply(
            &doc,
            &target,
            "m.py",
            None,
            &weave_core::host::Host::default(),
        )
        .expect("apply");
        assert!(!out.clean);
        assert_eq!(out.findings[0].kind.as_deref(), Some("both_modified"));
    }

    // ---- the decode boundary -----------------------------------------------

    /// Every way an ops document can fail to be one, refused by name.
    ///
    /// The point is not that these are rejected — three of the four were
    /// rejected before, as prose. It is that a caller can tell which happened
    /// without reading a sentence, and that the two that were *accepted*
    /// (an unknown field, an unknown verb) no longer are.
    #[test]
    fn a_malformed_ops_document_is_refused_with_a_matchable_error() {
        let good = r#"{
            "schema_version": "1.0.0",
            "file": "m.py",
            "exact": true,
            "ops": [{"op": "edited", "entity": "alpha", "entity_type": "function",
                     "content": "def alpha():\n    return 2\n"}]
        }"#;
        let doc = parse_ops_doc(good).expect("a well-formed document");
        assert_eq!(doc.ops[0].op, Op::Edited);

        // Not JSON at all.
        assert!(matches!(
            parse_ops_doc("this is not json"),
            Err(OpsDocError::Malformed { .. })
        ));

        // JSON, but missing a field the schema requires.
        let missing = r#"{"schema_version": "1.0.0", "file": "m.py", "ops": []}"#;
        assert!(matches!(
            parse_ops_doc(missing),
            Err(OpsDocError::Malformed { .. })
        ));

        // A field the schema does not define. `entitiy` used to be discarded
        // in silence and the op applied to an entity named "".
        let typo = good.replace("\"entity\":", "\"entitiy\":");
        assert!(
            matches!(parse_ops_doc(&typo), Err(OpsDocError::Malformed { .. })),
            "a misspelled field is not a document this build accepts"
        );

        // A verb outside the vocabulary. `eddited` used to decode and then
        // fall through the apply match into the "everything else" arm.
        let unknown_verb = good.replace("\"edited\"", "\"eddited\"");
        assert!(matches!(
            parse_ops_doc(&unknown_verb),
            Err(OpsDocError::Malformed { .. })
        ));

        // A major this build does not implement — a different thing entirely,
        // and the only one of the five a caller fixes by upgrading weave.
        let future = good.replace("1.0.0", "2.0.0");
        match parse_ops_doc(&future) {
            Err(OpsDocError::UnknownMajor { found }) => assert_eq!(found, "2.0.0"),
            other => panic!("expected UnknownMajor, got {other:?}"),
        }
    }

    /// The reserved relational verbs decode and are acted on by nobody — the
    /// schema's own rule for a v1 consumer, now stated in the type.
    #[test]
    fn a_reserved_verb_decodes_and_changes_nothing() {
        let doc = r#"{
            "schema_version": "1.0.0",
            "file": "m.py",
            "exact": true,
            "ops": [{"op": "moved", "entity": "alpha", "entity_type": "function"}]
        }"#;
        let doc = parse_ops_doc(doc).expect("reserved verbs are accepted");
        assert_eq!(doc.ops[0].op, Op::Moved);
        let out = apply_ops(BASE, "m.py", &doc);
        assert_eq!(out.content, BASE, "a reserved op is not acted on");
    }

    // ---- base integrity ---------------------------------------------------

    #[test]
    fn a_base_that_is_not_the_ops_base_is_refused_not_merged() {
        let changed = BASE.replace("return x * 2", "return x * 22");
        let doc = extract("m.py", BASE, &changed, false);
        let wrong_base = BASE.replace("def alpha(x):", "def alfa(x):");
        let err = apply(
            &doc,
            BASE,
            "m.py",
            Some(&wrong_base),
            &weave_core::host::Host::default(),
        )
        .unwrap_err();
        // The refusal is matchable, and both digests travel with it — the
        // caller does not have to re-hash anything to report what happened.
        let PatchError::BaseMismatch { expected, actual } = err;
        assert_eq!(expected, doc.base.as_ref().expect("base ref").sha256);
        assert_eq!(actual, sha256_hex(&wrong_base));
        assert_ne!(expected, actual);
    }

    #[test]
    fn with_no_base_available_the_target_stands_in_and_the_mode_says_so() {
        let changed = BASE.replace("return x * 2", "return x * 22");
        let doc = extract("m.py", BASE, &changed, false); // no inline base
        let out =
            apply(&doc, BASE, "m.py", None, &weave_core::host::Host::default()).expect("apply");
        assert_eq!(out.mode, ApplyMode::NoBase);
        assert_eq!(out.content, changed);
    }

    #[test]
    fn an_op_whose_entity_is_absent_is_reported_rather_than_dropped_silently() {
        let changed = BASE.replace("return x * 2", "return x * 22");
        let doc = extract("m.py", BASE, &changed, false);
        let target = "def alpha(x):\n    return x + 1\n"; // no beta at all
        let out = apply(
            &doc,
            target,
            "m.py",
            None,
            &weave_core::host::Host::default(),
        )
        .expect("apply");
        assert_eq!(out.unapplied, vec!["edited beta"]);
    }

    // ---- vocabulary -------------------------------------------------------

    #[test]
    fn v1_never_emits_unchanged_or_the_reserved_relational_ops() {
        let changed = BASE.replace("return x * 2", "return x * 22");
        let doc = extract("m.py", BASE, &changed, true);
        for op in &doc.ops {
            assert!(
                ["added", "edited", "renamed", "rename_edited", "deleted"].contains(&op.op.wire()),
                "unexpected op `{}`",
                op.op.wire()
            );
        }
    }

    #[test]
    fn a_consumer_tolerates_reserved_relational_ops_without_choking() {
        let mut doc = extract("m.py", BASE, BASE, true);
        doc.ops.push(PatchOp {
            op: Op::Moved,
            entity: "beta".into(),
            entity_type: "function".into(),
            from: None,
            to: None,
            content: None,
            after: None,
            separator: None,
            fallback: false,
        });
        let out = apply_ops(BASE, "m.py", &doc);
        assert_eq!(out.content, BASE, "a reserved op is accepted and ignored");
    }

    #[test]
    fn the_document_round_trips_through_json() {
        let changed = BASE.replace("return x * 2", "return x * 22");
        let doc = extract("m.py", BASE, &changed, true);
        let json = serde_json::to_string(&doc).unwrap();
        let back: OpsDoc = serde_json::from_str(&json).unwrap();
        assert_eq!(doc, back);
    }
}
