//! weave-patch-schema v1 — the READ side of weave's agent contract.
//!
//! The wire format is `schema/weave-findings.schema.json`; this module is the
//! only thing allowed to produce it. Everything here is a **mapping** from
//! weave-core's internal result types onto a small, versioned, finite
//! vocabulary that an agent can program against without knowing weave's
//! internals — and, critically, without weave's internals being frozen.
//!
//! Two vocabularies carry the whole contract:
//!
//! * **finding classes** — the fixed set of class tags this schema emits:
//!   DANGLING, DUP, SHADOW and ORDER for name-resolution changes, INTERLEAVE
//!   for effectful-order composition, LOSS for content no side dropped, ASYM
//!   for direction-dependence, and CONFLICT for the honest refusal. Callers
//!   program against these tags.
//! * **op vocabulary** — the per-entity actions. Seven per-entity actions plus
//!   four *relational* ones (split / merged / copied / moved) that a per-key
//!   matcher cannot emit. Those four are RESERVED: a v1 producer never emits
//!   them, a v1 consumer must tolerate them.
//!
//! Keeping the reserved half in v1 is deliberate — when weave's matcher grows
//! relational matching, consumers do not need a new major version.

use serde::Serialize;

use weave_cli::wire::{conflict_advice, Sides};
use weave_core::conflict::{ConflictKind, EntityConflict};
use weave_core::validate::{SemanticWarning, WarningKind};
use weave_core::{MergeResult, ResolutionStrategy};

pub(crate) const SCHEMA_VERSION: &str = "1.3.0";

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct Findings {
    pub schema_version: &'static str,
    pub file: String,
    pub clean: bool,
    pub confidence: &'static str,
    pub findings: Vec<Finding>,
    pub ops: Vec<Op>,
    pub bindings: Bindings,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct Finding {
    pub class: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<&'static str>,
    pub entity: String,
    pub entity_type: String,
    pub source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<&'static str>,
    /// The three-way alignment: which lines of base each side rewrote, where
    /// those rewrites meet, and what the meeting consists of (schema 1.1.0).
    /// Derived from the diffs weave already computed — never guessed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hunks: Vec<HunkRef>,
    /// How many hunks this conflict has. Greater than `hunks.len()` means the
    /// array was cut at `HUNK_LIMIT`; the ones carried are the first in base
    /// order.
    #[serde(skip_serializing_if = "is_zero")]
    pub hunks_total: usize,
    /// Which refusal fired. `both_modified` is a verdict, not a reason; this is
    /// the reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refused_by: Option<&'static str>,
    /// Where the three full texts are, rather than the three full texts
    /// (schema 1.1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub texts: Option<TextRefs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning_kind: Option<&'static str>,
    /// The members each side changed or added, when this finding is a
    /// sibling co-change advisory (schema 1.3.0). Present only on
    /// `class=COOCCUPANCY` / `source=advisory` findings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cochange: Option<CoChange>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<RelatedRef>,
}

/// Which members each side of a clean container merge changed or added — the
/// structured half of a sibling co-change advisory. The two lists never share a
/// member: a member both sides touched is a conflict or an identical edit, which
/// is not what this advisory reports.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct CoChange {
    pub ours: Vec<CoChangeMember>,
    pub theirs: Vec<CoChangeMember>,
}

/// One member one side changed or added.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct CoChangeMember {
    pub member: String,
    /// `added` | `changed`.
    pub change: &'static str,
}

/// One hunk of a conflict, as the wire states it.
///
/// `base_start` / `base_end` are 1-based inclusive line numbers **inside the
/// entity's own base text**, not inside the file. The finding is about an
/// entity; the entity's text is what `texts.base` references; and a range
/// stated against anything else would need a second coordinate system to be
/// checkable. `base_end < base_start` is a pure insertion before `base_start` —
/// an insertion has a position, not an extent.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct HunkRef {
    pub base_start: usize,
    pub base_end: usize,
    pub ours: SideRef,
    pub theirs: SideRef,
    /// How many base lines BOTH sides rewrote or deleted. Zero means the two
    /// edits are merely adjacent — one hunk because nothing separates them, not
    /// because they contradict.
    #[serde(skip_serializing_if = "is_zero")]
    pub collision_lines: usize,
    /// The first [`HUNK_SAMPLE`] of them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub collision: Vec<String>,
}

/// What one side did to one hunk.
///
/// `removed` / `added` are a SAMPLE, not the edit: at most
/// [`HUNK_SAMPLE`] leading lines each, with `removed_lines` / `added_lines`
/// giving the true extent. That is the whole size discipline of this document —
/// a finding's cost is a function of how many hunks a conflict has
/// and never of how big they are, so a 6,000-line file cannot produce a
/// half-megabyte document. The bytes that are not here are one `git show` away,
/// at the `path@rev` in `texts`, and the sample is enough to recognise the hunk
/// on sight without fetching anything.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct SideRef {
    /// `untouched` | `added` | `removed` | `modified`.
    pub action: &'static str,
    /// How many base lines this side deleted.
    #[serde(skip_serializing_if = "is_zero")]
    pub removed_lines: usize,
    /// How many lines this side wrote.
    #[serde(skip_serializing_if = "is_zero")]
    pub added_lines: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub added: Vec<String>,
    /// Identifiers this side's lines introduced, as a set difference over
    /// tokens. It names what appeared; it does not claim to know why. Capped at
    /// [`HUNK_TOKENS`], with `tokens_added_count` giving the true number.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tokens_added: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tokens_removed: Vec<String>,
    #[serde(skip_serializing_if = "is_zero")]
    pub tokens_added_count: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub tokens_removed_count: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// How many leading lines of each half of a hunk the document carries.
///
/// Two is enough to show a reader *what kind of thing* changed — the line the
/// hunk starts on and the one under it — without the document growing with the
/// file. The extent is always stated exactly; only the transcript is sampled.
const HUNK_SAMPLE: usize = 2;

/// How wide a sampled line may be before it is elided.
///
/// A sample is for recognising a hunk, not for reading it: past a hundred
/// characters it has stopped identifying anything and started being a copy of a
/// file the reader can fetch. The elision is marked, so nobody mistakes a
/// sample for the line.
const SAMPLE_WIDTH: usize = 100;

/// One sampled line, narrowed to what identifies it.
fn sample_line(l: &str) -> String {
    if l.chars().count() <= SAMPLE_WIDTH {
        return l.to_string();
    }
    let head: String = l.chars().take(SAMPLE_WIDTH).collect();
    format!("{head} \u{2026}")
}

/// How many identifiers of each direction the document carries.
const HUNK_TOKENS: usize = 8;

/// How many hunks of one conflict the document carries, in base order.
///
/// A large real-world conflict can have over 150 places where the two sides'
/// edits meet inside one class. Forty of them, in base order, with `hunks_total` stating
/// the true number, is the point where the document stops growing with the file
/// and starts being something an agent can read in one pass — which was the
/// complaint (`28KB-450KB docs broke naive Read/jq`). A reader who has worked
/// through forty hunks has the markers in front of them and does not need the
/// forty-first from here.
const HUNK_LIMIT: usize = 40;

/// Where the three versions of this entity's text are.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct TextRefs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<TextRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ours: Option<TextRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theirs: Option<TextRef>,
}

/// A reference to a text this document is NOT carrying.
///
/// `path@rev` is fetchable with one `git show`, which the reader already has;
/// `lines` and `bytes` say what it will cost; `content_hash` says whether the
/// entity's text is the one this finding was computed against. The hash is
/// `DefaultHasher` over the text and is a fingerprint, not a cryptographic
/// commitment — it answers "did this change under me", which is the question
/// asked here.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct TextRef {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    pub lines: usize,
    pub bytes: usize,
    pub content_hash: String,
}

/// The revisions the three sides came from, when the caller knows them.
///
/// `weave_findings` is asked about two branches and a merge base, so it always
/// does; the driver, which git hands three temp files, never does — and a `rev`
/// it invented would be worse than none.
#[derive(Debug, Clone, Default)]
pub(crate) struct Revs {
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct RelatedRef {
    pub entity: String,
    pub entity_type: String,
    pub file: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct Op {
    pub entity: String,
    pub entity_type: String,
    pub op: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub conflicted: bool,
    /// Never set on this side, and that is a gap rather than a tidiness.
    /// `weave patch extract` sets it on the WRITE side when entity granularity
    /// could not express a change. On the READ side a line-level fallback
    /// merge returns an EMPTY audit trail, so it produces no ops at all — the
    /// document is silent rather than flagged. The field is kept so the two
    /// sides stay one schema, and so that whoever gives the fallback route an
    /// audit trail has somewhere to put the answer.
    #[serde(skip_serializing_if = "is_false")]
    pub fallback: bool,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub(crate) struct Bindings {
    pub renames: Vec<Rename>,
    pub references: Vec<Reference>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct Rename {
    pub from: String,
    pub to: String,
    pub side: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct Reference {
    pub entity: String,
    pub defined_in_output: bool,
    pub referenced_in_output: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

// ---------------------------------------------------------------------------
// The mappings — the part that is actually contract
// ---------------------------------------------------------------------------

/// `ResolutionStrategy` → the op vocabulary.
///
/// Total over an exhaustive match: every variant maps, and conflicts map to the op
/// *shape* they would have had, flagged `conflicted` — so a caller iterating
/// `ops` sees every entity exactly once whether or not it conflicted.
pub(crate) fn op_of(r: &ResolutionStrategy) -> Op {
    let mut op = Op {
        entity: String::new(),
        entity_type: String::new(),
        op: "unchanged",
        from: None,
        to: None,
        conflicted: false,
        fallback: false,
    };
    match r {
        // Nothing changed, or both sides landed on the same content.
        ResolutionStrategy::Unchanged | ResolutionStrategy::ContentEqual => op.op = "unchanged",
        // Exactly one side changed the body, or both did and weave composed
        // them. From the caller's perspective these are all "edited".
        ResolutionStrategy::OursOnly
        | ResolutionStrategy::TheirsOnly
        | ResolutionStrategy::DiffyMerged
        | ResolutionStrategy::DecoratorMerged
        | ResolutionStrategy::InnerMerged
        | ResolutionStrategy::StatementMerged
        | ResolutionStrategy::FootprintLicensed => op.op = "edited",
        ResolutionStrategy::AddedOurs | ResolutionStrategy::AddedTheirs => op.op = "added",
        ResolutionStrategy::Deleted => op.op = "deleted",
        ResolutionStrategy::Renamed { from, to } => {
            op.op = "renamed";
            op.from = Some(from.clone());
            op.to = Some(to.clone());
        }
        // Conflicts: report the shape, mark it unresolved. The entity also
        // appears in `findings` with class=CONFLICT.
        ResolutionStrategy::ConflictBothModified | ResolutionStrategy::ConflictStatementScoped => {
            op.op = "edited";
            op.conflicted = true;
        }
        ResolutionStrategy::ConflictModifyDelete => {
            op.op = "deleted";
            op.conflicted = true;
        }
        ResolutionStrategy::ConflictBothAdded => {
            op.op = "added";
            op.conflicted = true;
        }
        ResolutionStrategy::ConflictRenameRename => {
            op.op = "renamed";
            op.conflicted = true;
        }
        ResolutionStrategy::ConflictRenameModify => {
            op.op = "rename_edited";
            op.conflicted = true;
        }
    }
    op
}

/// `WarningKind` → (finding class, wire name).
///
/// This is the one deliberately LOSSY edge in the schema: weave-core's warning
/// kinds are about *risk*, the class vocabulary is about *breakage*. The
/// original kind is therefore carried verbatim in `warning_kind`, so nothing
/// is destroyed by the translation.
pub(crate) fn class_of_warning(k: &WarningKind) -> (&'static str, &'static str) {
    match k {
        // A use may now bind against a contract that changed under it — the
        // rebinding class.
        WarningKind::DependencyAlsoModified => ("SHADOW", "dependency_also_modified"),
        WarningKind::DependentAlsoModified => ("SHADOW", "dependent_also_modified"),
        // The merged output no longer parses: structure was lost.
        WarningKind::ParseFailedAfterMerge => ("LOSS", "parse_failed_after_merge"),
        // Not a risk at all: a conflict that was resolved, with the evidence
        // that licensed it. It is on this channel because a caller is entitled
        // to know which of its clean entities weave composed under a rule
        // rather than found agreement on.
        WarningKind::CompositionLicensed { .. } => ("RESOLVED", "composition_licensed"),
        // The frame of a conflicted file states a line more times than any
        // union of the two sides could. That is the DUP class exactly — the
        // only difference from the clean-merge case is where it was found, and
        // the whole point of this check is that "where" stopped exempting it.
        WarningKind::ConflictFrameDuplicate { .. } => ("DUP", "conflict_frame_duplicate"),
        // A clean container merge in which each side changed different siblings.
        // Its own class, because it is neither a rebinding nor a loss nor a
        // duplicate — it is the co-occupancy of two disjoint edits, reported so a
        // human can confirm the coexistence weave cannot judge.
        WarningKind::SiblingCoChange { .. } => ("COOCCUPANCY", "sibling_co_change"),
    }
}

/// Which refusal fired, as a stable wire tag.
///
/// One owner: [`weave_core::ResolutionStrategy::guard`]. This table used to
/// live here, which meant the marker renderer — the thing every agent actually
/// reads — had no way to state the guard at all; the one part of weave's
/// output agents read unprompted was reachable only by opening a JSON
/// document. It moved into weave-core so both producers state the same name,
/// and this function is now the wire's re-export of that fact.
fn refused_by(strategy: &ResolutionStrategy) -> Option<&'static str> {
    strategy.guard()
}

/// One text, described rather than carried.
fn text_ref(path: &str, rev: Option<&String>, text: Option<&String>) -> Option<TextRef> {
    let t = text?;
    Some(TextRef {
        path: path.to_string(),
        rev: rev.cloned(),
        lines: t.lines().count(),
        bytes: t.len(),
        content_hash: format!("{:016x}", content_fingerprint(t)),
    })
}

/// A fingerprint of a text: "is this the version the finding was computed
/// against". Not a cryptographic commitment, and the schema says so.
fn content_fingerprint(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn finding_of_conflict(
    c: &EntityConflict,
    file: &str,
    revs: &Revs,
    strategy: Option<&ResolutionStrategy>,
) -> Finding {
    // The suggestion is the point of the finding: an agent that only learns
    // "there is a conflict" has learned nothing it could not see itself.
    //
    // Both the tag and the advice come from the one owner shared with the
    // patch reporter (`weave_cli::wire::conflict_advice`); the only thing this
    // producer chooses is that its two sides are called ours and theirs.
    let (kind, suggestion) = conflict_advice(c, Sides::MERGE);

    Finding {
        class: "CONFLICT",
        kind: Some(kind),
        entity: c.entity_name.clone(),
        entity_type: c.entity_type.clone(),
        source: "conflict",
        complexity: Some(c.complexity.to_string()),
        confidence: Some(confidence_of_complexity(&c.complexity)),
        // The three texts became three references and a diagnosis. What left
        // the document is what the reader already had —
        // `git show` reaches every byte of it, and the reference says exactly
        // which bytes. What arrived is the one thing they could not cheaply
        // compute: the alignment of the two sides against base.
        hunks_total: hunk_count(c),
        hunks: hunks_of(c),
        refused_by: strategy.and_then(refused_by),
        texts: Some(TextRefs {
            base: text_ref(file, revs.base.as_ref(), c.base_content.as_ref()),
            ours: text_ref(file, revs.ours.as_ref(), c.ours_content.as_ref()),
            theirs: text_ref(file, revs.theirs.as_ref(), c.theirs_content.as_ref()),
        }),
        suggestion: Some(suggestion),
        warning_kind: None,
        cochange: None,
        related: vec![],
    }
}

/// The per-hunk diagnosis, mapped onto the wire.
///
/// A conflict with no base — both sides added the same key divergently — has no
/// three-way alignment to state, and an empty `hunks` array is the honest answer
/// rather than a two-way diff dressed up as one.
fn hunk_count(c: &EntityConflict) -> usize {
    all_hunks(c).len()
}

fn hunks_of(c: &EntityConflict) -> Vec<HunkRef> {
    all_hunks(c)
        .into_iter()
        .take(HUNK_LIMIT)
        .map(|h| HunkRef {
            base_start: h.base_start,
            base_end: h.base_end,
            ours: side_ref(h.ours),
            theirs: side_ref(h.theirs),
            collision_lines: h.collision.len(),
            collision: h
                .collision
                .iter()
                .take(HUNK_SAMPLE)
                .map(|l| sample_line(l))
                .collect(),
        })
        .collect()
}

fn all_hunks(c: &EntityConflict) -> Vec<weave_core::diagnose::Hunk> {
    let (Some(base), Some(ours), Some(theirs)) = (
        c.base_content.as_deref(),
        c.ours_content.as_deref(),
        c.theirs_content.as_deref(),
    ) else {
        return Vec::new();
    };
    weave_core::diagnose::diagnose(base, ours, theirs)
}

fn side_ref(e: weave_core::diagnose::SideEdit) -> SideRef {
    let (removed_lines, added_lines) = (e.removed.len(), e.added.len());
    let (ta, tr) = (e.tokens_added.len(), e.tokens_removed.len());
    let cap = |mut v: Vec<String>, n: usize| {
        v.truncate(n);
        v
    };
    let sample = |v: Vec<String>, n: usize| -> Vec<String> {
        v.into_iter().take(n).map(|l| sample_line(&l)).collect()
    };
    SideRef {
        action: e.action.wire(),
        removed_lines,
        added_lines,
        removed: sample(e.removed, HUNK_SAMPLE),
        added: sample(e.added, HUNK_SAMPLE),
        tokens_added: cap(e.tokens_added, HUNK_TOKENS),
        tokens_removed: cap(e.tokens_removed, HUNK_TOKENS),
        tokens_added_count: ta,
        tokens_removed_count: tr,
    }
}

fn confidence_of_complexity(c: &weave_core::conflict::ConflictComplexity) -> &'static str {
    use weave_core::conflict::ConflictComplexity as C;
    match c {
        C::Text => "high",
        C::Syntax | C::Functional | C::TextSyntax | C::TextFunctional => "medium",
        C::SyntaxFunctional | C::TextSyntaxFunctional => "low",
        C::Unknown => "unknown",
    }
}

fn finding_of_warning(w: &SemanticWarning) -> Finding {
    let (class, wire_kind) = class_of_warning(&w.kind);
    // A co-change is the lowest register on this channel: not a warning about
    // risk, an advisory about a fact. Everything else here is `warning`.
    let source = match &w.kind {
        WarningKind::SiblingCoChange { .. } => "advisory",
        _ => "warning",
    };
    let suggestion = match &w.kind {
        WarningKind::ParseFailedAfterMerge => {
            "The merged file no longer parses. Do not commit it — re-run the merge \
             with conflicts, or resolve by hand."
                .to_string()
        }
        WarningKind::CompositionLicensed {
            ours_binds,
            theirs_binds,
        } => format!(
            "Both sides inserted into one slot of `{}` and weave kept both: ours binds \
             [{}], theirs binds [{}], and neither block writes what the other writes or \
             reads. Check that reading before trusting it.",
            w.entity_name,
            ours_binds.join(", "),
            theirs_binds.join(", "),
        ),
        WarningKind::SiblingCoChange {
            ours_added,
            ours_changed,
            theirs_added,
            theirs_changed,
        } => format!(
            "`{}` merged clean, but both sides changed different siblings ({}; {}). \
             Nothing conflicted — confirm the two sets of members are meant to coexist \
             before trusting the merge.",
            w.entity_name,
            weave_core::validate::co_change_side_phrase("ours", ours_added, ours_changed),
            weave_core::validate::co_change_side_phrase("theirs", theirs_added, theirs_changed),
        ),
        _ => format!(
            "`{}` was auto-merged and is coupled to another entity changed in this \
             same merge. Re-read both together before trusting the result.",
            w.entity_name
        ),
    };
    let cochange = match &w.kind {
        WarningKind::SiblingCoChange {
            ours_added,
            ours_changed,
            theirs_added,
            theirs_changed,
        } => Some(CoChange {
            ours: cochange_members(ours_added, ours_changed),
            theirs: cochange_members(theirs_added, theirs_changed),
        }),
        _ => None,
    };
    Finding {
        class,
        kind: None,
        entity: w.entity_name.clone(),
        entity_type: w.entity_type.clone(),
        source,
        complexity: None,
        confidence: None,
        hunks: Vec::new(),
        hunks_total: 0,
        refused_by: None,
        texts: None,
        suggestion: Some(suggestion),
        warning_kind: Some(wire_kind),
        cochange,
        related: w
            .related
            .iter()
            .map(|r| RelatedRef {
                entity: r.name.clone(),
                entity_type: r.entity_type.clone(),
                file: r.file_path.clone(),
            })
            .collect(),
    }
}

/// The structured member list for one side: additions then changes, each tagged.
fn cochange_members(added: &[String], changed: &[String]) -> Vec<CoChangeMember> {
    added
        .iter()
        .map(|m| CoChangeMember {
            member: m.clone(),
            change: "added",
        })
        .chain(changed.iter().map(|m| CoChangeMember {
            member: m.clone(),
            change: "changed",
        }))
        .collect()
}

// ---------------------------------------------------------------------------
// Binding facts — the read-only form of the binding pass
// ---------------------------------------------------------------------------

/// Is `name` *defined* in the merged output, and is it *used* there?
///
/// Both questions are weave-core's to answer ([`weave_core::binding`]). This
/// module used to carry its own looser pair of predicates — substring matching
/// that counted `db.fetch_user(` as a call to `fetch_user` — which meant the
/// DANGLING findings reported here were computed by different rules than the
/// dangling repairs weave-core's binding pass had already performed. Two
/// answers to one question is how a document comes to contradict the merge it
/// describes.
use weave_core::binding::{has_call_reference as is_referenced, has_definition as is_defined};

/// Which side performed a rename, decided by where the new name actually
/// appears. `both` when both sides converged on it; `base` should not occur.
fn rename_side(to: &str, ours: &str, theirs: &str) -> &'static str {
    match (is_defined(ours, to), is_defined(theirs, to)) {
        (true, true) => "both",
        (true, false) => "ours",
        (false, true) => "theirs",
        (false, false) => "both",
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Build the v1 findings document for one merged file.
///
/// `ours` / `theirs` are the pre-merge sides; they are needed only to attribute
/// renames to a side, which the audit trail alone cannot say.
pub(crate) fn build(
    file: &str,
    result: &MergeResult,
    ours: &str,
    theirs: &str,
    revs: &Revs,
) -> Findings {
    // The refusal that produced each conflict, joined from the audit trail by
    // the pair that identifies an entity in this document already. The audit is
    // where the strategy lives; the conflict is where the verdict lives; the
    // reader is owed both.
    let strategy_of: std::collections::BTreeMap<(&str, &str), &ResolutionStrategy> = result
        .audit
        .iter()
        .map(|a| ((a.name.as_str(), a.entity_type.as_str()), &a.resolution))
        .collect();
    let mut findings: Vec<Finding> = Vec::new();
    let mut ops: Vec<Op> = Vec::new();
    let mut renames: Vec<Rename> = Vec::new();
    let mut references: Vec<Reference> = Vec::new();

    for a in &result.audit {
        let mut op = op_of(&a.resolution);
        op.entity = a.name.clone();
        op.entity_type = a.entity_type.clone();
        if let (Some(from), Some(to)) = (op.from.clone(), op.to.clone()) {
            renames.push(Rename {
                from,
                to: to.clone(),
                side: rename_side(&to, ours, theirs),
            });
        }
        ops.push(op);
    }

    // Rename facts also live inside conflicts, where the side is stated
    // outright rather than inferred.
    for c in &result.conflicts {
        match &c.kind {
            ConflictKind::RenameModify {
                old_name,
                new_name,
                renamed_in_ours,
            } => renames.push(Rename {
                from: old_name.clone(),
                to: new_name.clone(),
                side: if *renamed_in_ours { "ours" } else { "theirs" },
            }),
            ConflictKind::RenameRename {
                base_name,
                ours_name,
                theirs_name,
            } => {
                renames.push(Rename {
                    from: base_name.clone(),
                    to: ours_name.clone(),
                    side: "ours",
                });
                renames.push(Rename {
                    from: base_name.clone(),
                    to: theirs_name.clone(),
                    side: "theirs",
                });
            }
            _ => {}
        }
        let strategy = strategy_of
            .get(&(c.entity_name.as_str(), c.entity_type.as_str()))
            .copied();
        findings.push(finding_of_conflict(c, file, revs, strategy));
    }

    for w in &result.warnings {
        findings.push(finding_of_warning(w));
    }

    // Binding facts over the merged output, plus the two classes they decide.
    let mut names: Vec<String> = ops
        .iter()
        .map(|o| o.entity.clone())
        .chain(renames.iter().flat_map(|r| [r.from.clone(), r.to.clone()]))
        .collect();
    names.sort();
    names.dedup();

    for name in &names {
        if name.is_empty() || name == "(file)" {
            continue;
        }
        let defined = is_defined(&result.content, name);
        let referenced = is_referenced(&result.content, name);
        references.push(Reference {
            entity: name.clone(),
            defined_in_output: defined,
            referenced_in_output: referenced,
        });

        // DANGLING: a use survived its definition. This is the whole reason
        // renames must be reported as facts and not just as prose.
        if referenced && !defined && result.is_clean() {
            let target = renames.iter().find(|r| &r.from == name).map(|r| &r.to);
            findings.push(Finding {
                class: "DANGLING",
                kind: None,
                entity: name.clone(),
                entity_type: "unknown".into(),
                source: "derived",
                complexity: None,
                confidence: Some("high"),
                hunks: Vec::new(),
                hunks_total: 0,
                refused_by: None,
                texts: None,
                suggestion: Some(match target {
                    Some(to) => format!(
                        "`{name}` is still called but no longer defined — it was renamed to \
                         `{to}`. Rewrite the remaining call sites."
                    ),
                    None => format!(
                        "`{name}` is still called but no longer defined. Restore it or remove \
                         the call sites."
                    ),
                }),
                warning_kind: None,
                cochange: None,
                related: vec![],
            });
        }
    }

    // DUP: one name defined more than once in a clean output.
    if result.is_clean() {
        for name in &names {
            if name.is_empty() {
                continue;
            }
            let n = result
                .content
                .lines()
                .filter(|l| {
                    let t = l.trim();
                    t.starts_with(&format!("def {name}("))
                        || t.starts_with(&format!("class {name}"))
                        || t.contains(&format!("function {name}("))
                        || t.starts_with(&format!("fn {name}("))
                        || t.starts_with(&format!("pub fn {name}("))
                        || t.starts_with(&format!("func {name}("))
                })
                .count();
            if n > 1 {
                findings.push(Finding {
                    class: "DUP",
                    kind: None,
                    entity: name.clone(),
                    entity_type: "unknown".into(),
                    source: "derived",
                    complexity: None,
                    confidence: Some("high"),
                    hunks: Vec::new(),
                    hunks_total: 0,
                    refused_by: None,
                    texts: None,
                    suggestion: Some(format!(
                        "`{name}` is defined {n} times in the merged output. Keep one \
                         definition; the duplicate is a merge artifact, not an authored change."
                    )),
                    warning_kind: None,
                    cochange: None,
                    related: vec![],
                });
            }
        }
    }

    renames.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));
    renames.dedup();

    Findings {
        schema_version: SCHEMA_VERSION,
        file: file.to_string(),
        clean: result.is_clean(),
        confidence: result.stats.confidence(),
        findings,
        ops,
        bindings: Bindings {
            renames,
            references,
        },
    }
}

// ===========================================================================
// Tests: the MAPPING is the contract, so the mapping is what is tested.
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Every op the schema declares, as a set. Kept literal so a schema edit
    /// and a code edit cannot silently drift apart.
    const OP_VOCABULARY: [&str; 10] = [
        "added",
        "unchanged",
        "edited",
        "renamed",
        "rename_edited",
        "deleted",
        "split",
        "merged",
        "copied",
        "moved",
    ];

    const FINDING_CLASSES: [&str; 9] = [
        "CONFLICT",
        "LOSS",
        "DUP",
        "DANGLING",
        "SHADOW",
        "ORDER",
        "INTERLEAVE",
        "ASYM",
        "COOCCUPANCY",
    ];

    /// Every ResolutionStrategy variant, exhaustively. If weave-core adds one,
    /// `op_of`'s match stops compiling and this list stops being complete —
    /// which is the point.
    fn all_strategies() -> Vec<ResolutionStrategy> {
        vec![
            ResolutionStrategy::Unchanged,
            ResolutionStrategy::OursOnly,
            ResolutionStrategy::TheirsOnly,
            ResolutionStrategy::ContentEqual,
            ResolutionStrategy::DiffyMerged,
            ResolutionStrategy::DecoratorMerged,
            ResolutionStrategy::InnerMerged,
            ResolutionStrategy::StatementMerged,
            ResolutionStrategy::FootprintLicensed,
            ResolutionStrategy::ConflictBothModified,
            ResolutionStrategy::ConflictStatementScoped,
            ResolutionStrategy::ConflictModifyDelete,
            ResolutionStrategy::ConflictBothAdded,
            ResolutionStrategy::ConflictRenameRename,
            ResolutionStrategy::ConflictRenameModify,
            ResolutionStrategy::AddedOurs,
            ResolutionStrategy::AddedTheirs,
            ResolutionStrategy::Deleted,
            ResolutionStrategy::Renamed {
                from: "old".into(),
                to: "new".into(),
            },
        ]
    }

    #[test]
    fn every_resolution_strategy_maps_into_the_op_vocabulary() {
        for s in all_strategies() {
            let op = op_of(&s);
            assert!(
                OP_VOCABULARY.contains(&op.op),
                "{s:?} mapped to {:?}, which is not in the schema's op vocabulary",
                op.op
            );
        }
    }

    #[test]
    fn conflict_strategies_are_flagged_and_resolved_ones_are_not() {
        for s in all_strategies() {
            let op = op_of(&s);
            let is_conflict = format!("{s:?}").starts_with("Conflict");
            assert_eq!(
                op.conflicted, is_conflict,
                "{s:?}: conflicted flag must mirror the strategy"
            );
        }
    }

    #[test]
    fn rename_carries_both_names_and_nothing_else_does() {
        let op = op_of(&ResolutionStrategy::Renamed {
            from: "load_user".into(),
            to: "fetch_user".into(),
        });
        assert_eq!(op.op, "renamed");
        assert_eq!(op.from.as_deref(), Some("load_user"));
        assert_eq!(op.to.as_deref(), Some("fetch_user"));

        for s in all_strategies() {
            if matches!(s, ResolutionStrategy::Renamed { .. }) {
                continue;
            }
            let op = op_of(&s);
            assert!(
                op.from.is_none() && op.to.is_none(),
                "{s:?} must not claim a rename"
            );
        }
    }

    #[test]
    fn every_conflict_kind_maps_into_the_conflict_kind_vocabulary() {
        let kinds = [
            ConflictKind::BothModified,
            ConflictKind::ModifyDelete {
                modified_in_ours: true,
            },
            ConflictKind::BothAdded,
            ConflictKind::RenameRename {
                base_name: "a".into(),
                ours_name: "b".into(),
                theirs_name: "c".into(),
            },
            ConflictKind::RenameModify {
                old_name: "a".into(),
                new_name: "b".into(),
                renamed_in_ours: true,
            },
        ];
        let allowed = [
            "both_modified",
            "modify_delete",
            "both_added",
            "rename_rename",
            "rename_modify",
        ];
        for k in &kinds {
            assert!(allowed.contains(&k.wire_kind()), "unmapped: {k:?}");
        }
    }

    #[test]
    fn warning_kinds_map_into_the_class_vocabulary_without_losing_the_original() {
        for k in [
            WarningKind::DependencyAlsoModified,
            WarningKind::DependentAlsoModified,
            WarningKind::ParseFailedAfterMerge,
            WarningKind::SiblingCoChange {
                ours_added: vec!["bar".into()],
                ours_changed: vec![],
                theirs_added: vec![],
                theirs_changed: vec!["baz".into()],
            },
        ] {
            let (class, wire) = class_of_warning(&k);
            assert!(
                FINDING_CLASSES.contains(&class),
                "{k:?} mapped to unknown class {class}"
            );
            assert!(
                !wire.is_empty(),
                "{k:?} must keep its original kind on the wire"
            );
        }
    }

    /// The advisory is the lowest register: a fact about a CLEAN merge. It must
    /// map to `source=advisory`, keep `clean=true`, and carry the two disjoint
    /// member sets structurally — none of which a conflict finding does.
    #[test]
    fn a_sibling_co_change_maps_to_a_clean_advisory_with_both_member_sets() {
        let w = SemanticWarning {
            entity_name: "Foo".into(),
            entity_type: "class".into(),
            file_path: "app.py".into(),
            kind: WarningKind::SiblingCoChange {
                ours_added: vec!["bar".into()],
                ours_changed: vec![],
                theirs_added: vec![],
                theirs_changed: vec!["baz".into()],
            },
            related: vec![],
        };
        let f = finding_of_warning(&w);
        assert_eq!(f.class, "COOCCUPANCY");
        assert_eq!(f.source, "advisory");
        assert_eq!(f.warning_kind, Some("sibling_co_change"));
        let c = f.cochange.expect("advisory carries its member sets");
        assert_eq!(
            c.ours,
            vec![CoChangeMember {
                member: "bar".into(),
                change: "added"
            }]
        );
        assert_eq!(
            c.theirs,
            vec![CoChangeMember {
                member: "baz".into(),
                change: "changed"
            }]
        );
        let s = f.suggestion.unwrap();
        assert!(
            s.contains("bar") && s.contains("baz") && s.contains("coexist"),
            "{s}"
        );
    }

    // --- end-to-end over a real merge -------------------------------------

    #[test]
    fn a_clean_disjoint_merge_produces_ops_and_no_findings() {
        let base = "def alpha():\n    return 1\n";
        let ours = "def alpha():\n    return 1\n\n\ndef ours_only():\n    return 2\n";
        let theirs = "def alpha():\n    return 1\n\n\ndef theirs_only():\n    return 3\n";
        let result = weave_core::entity_merge(base, ours, theirs, "app.py");
        let f = build("app.py", &result, ours, theirs, &Revs::default());

        assert_eq!(f.schema_version, "1.3.0");
        assert!(f.clean);
        assert!(
            f.findings.is_empty(),
            "a disjoint clean merge should have nothing to report, got {:?}",
            f.findings
        );
        assert!(!f.ops.is_empty(), "ops must describe every entity");
        for op in &f.ops {
            assert!(OP_VOCABULARY.contains(&op.op));
            assert!(!op.entity.is_empty() && !op.entity_type.is_empty());
        }
        // Every entity in the output is accounted for as defined.
        assert!(f
            .bindings
            .references
            .iter()
            .any(|r| r.entity == "alpha" && r.defined_in_output));
    }

    #[test]
    fn a_conflicted_merge_reports_a_conflict_finding_with_all_three_sides() {
        let base = "def alpha():\n    return 1\n";
        let ours = "def alpha():\n    ours_body()\n    return 1\n";
        let theirs = "def alpha():\n    theirs_body()\n    return 1\n";
        let result = weave_core::entity_merge(base, ours, theirs, "app.py");
        assert!(!result.is_clean(), "fixture must conflict");

        let f = build("app.py", &result, ours, theirs, &Revs::default());
        assert!(!f.clean);
        let c = f
            .findings
            .iter()
            .find(|x| x.class == "CONFLICT")
            .expect("a conflicted merge must yield a CONFLICT finding");
        assert_eq!(c.entity, "alpha");
        assert_eq!(c.source, "conflict");
        assert!(c.kind.is_some(), "CONFLICT findings must state their kind");
        // Schema 1.1.0: the three texts became three REFERENCES and a
        // diagnosis. Both sides are still required — the finding must say where
        // each one is, and what each one did.
        let t = c
            .texts
            .as_ref()
            .expect("a CONFLICT finding must locate its texts");
        assert!(
            t.ours.is_some() && t.theirs.is_some(),
            "both sides required"
        );
        assert!(
            !c.hunks.is_empty(),
            "a both-modified conflict must state which hunks differ"
        );
        assert!(
            c.refused_by.is_some(),
            "a conflict must say which refusal fired, not only that one did"
        );
        assert!(
            c.suggestion.as_deref().unwrap_or("").len() > 10,
            "a finding without a repair line is just an error message"
        );
        // The op record covers the same entity, flagged.
        assert!(f.ops.iter().any(|o| o.entity == "alpha" && o.conflicted));
    }

    #[test]
    fn the_document_round_trips_through_json_with_the_schemas_key_names() {
        let base = "def alpha():\n    return 1\n";
        let ours = "def alpha():\n    ours_body()\n    return 1\n";
        let theirs = "def alpha():\n    theirs_body()\n    return 1\n";
        let result = weave_core::entity_merge(base, ours, theirs, "app.py");
        let f = build("app.py", &result, ours, theirs, &Revs::default());

        let v = serde_json::to_value(&f).expect("must serialize");
        for key in [
            "schema_version",
            "file",
            "clean",
            "confidence",
            "findings",
            "ops",
            "bindings",
        ] {
            assert!(v.get(key).is_some(), "missing top-level key {key:?}");
        }
        assert!(v["bindings"].get("renames").is_some());
        assert!(v["bindings"].get("references").is_some());
        // Optional fields must be omitted, not null — consumers distinguish
        // "weave has nothing to say" from "weave says null".
        let first = &v["findings"][0];
        assert!(
            first.get("warning_kind").is_none(),
            "absent optional fields must be omitted entirely"
        );
    }

    #[test]
    fn binding_facts_expose_dangling_as_a_derived_finding() {
        // ours renames the callee; theirs adds a caller of the OLD name.
        let base = "def load_user(i):\n    return i\n";
        let ours = "def fetch_user(i):\n    return i\n";
        let theirs =
            "def load_user(i):\n    return i\n\n\ndef report():\n    return load_user(1)\n";
        let result = weave_core::entity_merge(base, ours, theirs, "app.py");
        let f = build("app.py", &result, ours, theirs, &Revs::default());

        if !f.clean {
            return; // an honest conflict needs no derived finding
        }
        let refs: Vec<&Reference> = f.bindings.references.iter().collect();
        assert!(!refs.is_empty(), "binding facts must be reported");
        if let Some(r) = refs.iter().find(|r| r.entity == "load_user") {
            if r.referenced_in_output && !r.defined_in_output {
                let d = f
                    .findings
                    .iter()
                    .find(|x| x.class == "DANGLING" && x.entity == "load_user")
                    .expect("referenced-but-undefined must surface as DANGLING");
                assert_eq!(d.source, "derived");
                assert!(d.suggestion.is_some());
            }
        }
    }

    #[test]
    fn reserved_relational_ops_are_declared_but_never_emitted_by_v1() {
        // weave's matcher is injective today: split/merged/copied/moved are in
        // the vocabulary so consumers are forward-compatible, but no
        // ResolutionStrategy produces them yet. If that changes, this test
        // fails and the contract doc must be updated with it.
        let reserved = ["split", "merged", "copied", "moved"];
        for s in all_strategies() {
            assert!(
                !reserved.contains(&op_of(&s).op),
                "{s:?} now emits a relational op — update this module's doc comment \
                 and the wire schema to match"
            );
        }
    }
}
